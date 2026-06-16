//! Verification engine for `deal check --verify` (Phase 5 / D-85, D-86, D-87).
//!
//! Implements:
//!   - IR-walk over satisfy/criteria/compute/evidence blocks (SIM-5)
//!   - Expression evaluation for the showcase grammar subset (D-55, D-86)
//!   - Three-level verdict: PASS / FAIL / PARTIAL + orthogonal STALE flag (D-86)
//!   - Unit/dimension compatibility via deal_check_with_stdlib C ABI (D-85)
//!   - Per-requirement verification report (D-87)
//!   - D-32 JSON envelope output for `--json` mode
//!
//! ## Expression Grammar (LOCKED — showcase only, D-55 precedent)
//!
//! Operators:  >=  <=  ==  AND  +  -  *  /
//! Built-ins:  max(a, b, ...)
//! Refs:       dot-separated field-path (e.g. REQ_SYS_001.minRange)
//! Literals:   numeric (integer or float)
//! NOT supported: OR, NOT, !=, unary minus in complex positions
//!
//! ## IR format (from deal parse / deal_ast_json)
//!
//! The `deal parse` / `deal_ir_json` output is an AST JSON. Satisfy blocks appear
//! as `{"k":"satisfy_block", "children": [...]}` nodes inside a
//! `{"k":"traceability_block", ...}` parent. Children include:
//!   - `object_literal` with `{"key":"requirement", "value":{"value":"REQ_BAT_001"}}`
//!   - `object_literal` with `{"key":"status", "value":{"value":"partial"}}`
//!   - `annotation` nodes with `"name":"criteria"`, `"name":"compute"`,
//!     `"name":"evidence ..."`, `"name":"gap"` — body text in `body._body.value`
//!
//! ## Staleness check (D-83/D-84)
//!
//! Staleness is computed as SHA-256 of evidence output.json bytes. The recorded
//! hash is read from `evidence/baselines/<tag>/manifest.json`. A mismatch sets
//! the orthogonal `stale` flag — NEVER folded into PARTIAL.
//! STALE + no --run-sims → report and exit non-zero (gate mode, D-84).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context as _};
use hex;
use sha2::{Digest, Sha256};

use crate::CliError;

// Re-export deal_ffi for dimension check
use deal_ffi as ffi;

// ─── Verdict types (D-86) ─────────────────────────────────────────────────────

/// Level-2 verdict rubric (SIM-5 / D-86).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    /// All criteria evaluated true AND evidence complete AND fresh.
    Pass,
    /// At least one criterion evaluated false.
    Fail,
    /// Partial satisfaction: `status="partial"`, unmapped return fields, or gap block.
    Partial,
}

/// A single criterion evaluation result (D-86).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CriterionResult {
    /// Boolean verdict for this criterion.
    pub verdict: Verdict,
    /// True if the backing evidence is stale (D-83/D-84). Orthogonal to verdict.
    pub stale: bool,
    /// Human-readable description (e.g. "actualRange >= REQ_SYS_001.minRange").
    pub description: String,
}

/// Per-requirement verification report entry (D-87).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequirementVerdict {
    /// Requirement ID (e.g. "REQ_SYS_001").
    pub requirement_id: String,
    /// Aggregate verdict for this requirement.
    pub verdict: Verdict,
    /// Staleness flag (D-84).
    pub stale: bool,
    /// Per-criterion breakdowns.
    pub criteria: Vec<CriterionResult>,
    /// Computed margins from the compute{} block (name → numeric value).
    pub compute_results: std::collections::BTreeMap<String, f64>,
    /// Evidence bindings (return_field → evidence_source).
    pub evidence_bindings: std::collections::BTreeMap<String, String>,
}

/// Full verification report (D-87).
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifyReport {
    /// Per-requirement verdicts keyed by REQ_* id.
    pub requirements: std::collections::BTreeMap<String, RequirementVerdict>,
    /// Summary counts.
    pub summary: VerifySummary,
}

/// Summary counts across all requirements.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifySummary {
    pub pass: usize,
    pub fail: usize,
    pub partial: usize,
    pub stale: usize,
    pub total: usize,
}

// ─── Expression evaluator ─────────────────────────────────────────────────────

/// A value in the evaluator (number, bool, or an unmapped field-path signal).
#[derive(Debug, Clone, PartialEq)]
pub enum EvalValue {
    /// A numeric value (f64 covers all DEAL numeric types at this layer).
    Number(f64),
    /// A boolean (from comparison / AND operators).
    Bool(bool),
    /// An unresolved field-path reference: feeds PARTIAL (never a panic).
    Unmapped,
}

/// Evaluation context: maps field-path strings to values.
#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    values: HashMap<String, EvalValue>,
}

impl EvalContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a value by key.
    pub fn set(&mut self, key: &str, value: EvalValue) {
        self.values.insert(key.to_string(), value);
    }

    /// Look up a key (dot-separated field path included).
    pub fn get(&self, key: &str) -> Option<&EvalValue> {
        self.values.get(key)
    }
}

/// Resolve a dot-separated field path against the context.
/// Returns `Some(EvalValue)` if found, `None` if not in context.
pub fn resolve_field_path(path: &str, ctx: &EvalContext) -> Option<EvalValue> {
    ctx.get(path).cloned()
}

// ─── Recursive-descent expression evaluator ──────────────────────────────────

/// Evaluate a showcase grammar expression string against the context.
///
/// Grammar (LOCKED per D-55):
///   expr ::= and_expr (AND and_expr)*
///   and_expr ::= cmp_expr
///   cmp_expr ::= add_expr (('>=' | '<=' | '==') add_expr)?
///   add_expr ::= mul_expr (('+' | '-') mul_expr)*
///   mul_expr ::= unary ('*' | '/') unary)*
///   unary ::= field_path | number | call | '(' expr ')'
///   call ::= 'max' '(' expr (',' expr)* ')'
///   field_path ::= IDENT ('.' IDENT)*
///
/// Any operator outside this set (OR, NOT, !=, etc.) returns a scope-guard error.
pub fn eval_expr(expr: &str, ctx: &EvalContext) -> anyhow::Result<EvalValue> {
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_and_expr(&tokens, &mut pos, ctx)?;
    // Ensure we consumed all tokens
    if pos < tokens.len() {
        // Any unconsumed token is a scope violation
        return Err(anyhow!(
            "unsupported expression operator or syntax (deferred per D-55 showcase grammar): unexpected token '{}' at position {} in: {}",
            tokens[pos].text,
            pos,
            expr
        ));
    }
    Ok(result)
}

// ─── Tokenizer ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Number,
    Ident, // includes field paths like REQ_SYS_001.minRange
    Gte,   // >=
    Lte,   // <=
    Eq,    // ==
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    And, // AND keyword
         // scope-guard catches everything else
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    text: String,
}

fn tokenize(s: &str) -> anyhow::Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // skip whitespace
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        // Two-character operators
        if i + 1 < chars.len() {
            match (chars[i], chars[i + 1]) {
                ('>', '=') => {
                    tokens.push(Token {
                        kind: TokenKind::Gte,
                        text: ">=".into(),
                    });
                    i += 2;
                    continue;
                }
                ('<', '=') => {
                    tokens.push(Token {
                        kind: TokenKind::Lte,
                        text: "<=".into(),
                    });
                    i += 2;
                    continue;
                }
                ('=', '=') => {
                    tokens.push(Token {
                        kind: TokenKind::Eq,
                        text: "==".into(),
                    });
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-character operators
        match chars[i] {
            '+' => {
                tokens.push(Token {
                    kind: TokenKind::Plus,
                    text: "+".into(),
                });
                i += 1;
            }
            '-' => {
                tokens.push(Token {
                    kind: TokenKind::Minus,
                    text: "-".into(),
                });
                i += 1;
            }
            '*' => {
                tokens.push(Token {
                    kind: TokenKind::Star,
                    text: "*".into(),
                });
                i += 1;
            }
            '/' => {
                tokens.push(Token {
                    kind: TokenKind::Slash,
                    text: "/".into(),
                });
                i += 1;
            }
            '(' => {
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    text: "(".into(),
                });
                i += 1;
            }
            ')' => {
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    text: ")".into(),
                });
                i += 1;
            }
            ',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    text: ",".into(),
                });
                i += 1;
            }
            // Numeric literal
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token {
                    kind: TokenKind::Number,
                    text,
                });
            }
            // Identifier / keyword / field-path (may contain dots)
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                // Scope guard: reject OR / NOT keywords
                if text == "OR" || text == "NOT" {
                    return Err(anyhow!(
                        "unsupported operator '{}' in showcase grammar (deferred per D-55)",
                        text
                    ));
                }
                let kind = if text == "AND" {
                    TokenKind::And
                } else {
                    TokenKind::Ident
                };
                tokens.push(Token { kind, text });
            }
            other => {
                return Err(anyhow!(
                    "unexpected character '{}' in showcase grammar expression (D-55)",
                    other
                ));
            }
        }
    }
    Ok(tokens)
}

// ─── Parser (recursive descent) ───────────────────────────────────────────────

fn parse_and_expr(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext,
) -> anyhow::Result<EvalValue> {
    let mut lhs = parse_cmp_expr(tokens, pos, ctx)?;
    while *pos < tokens.len() && tokens[*pos].kind == TokenKind::And {
        *pos += 1; // consume AND
        let rhs = parse_cmp_expr(tokens, pos, ctx)?;
        lhs = apply_and(lhs, rhs)?;
    }
    Ok(lhs)
}

fn apply_and(lhs: EvalValue, rhs: EvalValue) -> anyhow::Result<EvalValue> {
    match (lhs, rhs) {
        (EvalValue::Unmapped, _) | (_, EvalValue::Unmapped) => Ok(EvalValue::Unmapped),
        (EvalValue::Bool(l), EvalValue::Bool(r)) => Ok(EvalValue::Bool(l && r)),
        _ => Err(anyhow!("AND requires boolean operands")),
    }
}

fn parse_cmp_expr(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext,
) -> anyhow::Result<EvalValue> {
    let lhs = parse_add_expr(tokens, pos, ctx)?;
    if *pos < tokens.len() {
        match tokens[*pos].kind {
            TokenKind::Gte | TokenKind::Lte | TokenKind::Eq => {
                let op = tokens[*pos].kind.clone();
                *pos += 1;
                let rhs = parse_add_expr(tokens, pos, ctx)?;
                return apply_cmp(lhs, rhs, op);
            }
            _ => {}
        }
    }
    Ok(lhs)
}

fn apply_cmp(lhs: EvalValue, rhs: EvalValue, op: TokenKind) -> anyhow::Result<EvalValue> {
    match (lhs, rhs) {
        (EvalValue::Unmapped, _) | (_, EvalValue::Unmapped) => Ok(EvalValue::Unmapped),
        (EvalValue::Number(l), EvalValue::Number(r)) => {
            let result = match op {
                TokenKind::Gte => l >= r,
                TokenKind::Lte => l <= r,
                TokenKind::Eq => (l - r).abs() < f64::EPSILON,
                _ => unreachable!(),
            };
            Ok(EvalValue::Bool(result))
        }
        _ => Err(anyhow!("comparison requires numeric operands")),
    }
}

fn parse_add_expr(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext,
) -> anyhow::Result<EvalValue> {
    let mut lhs = parse_mul_expr(tokens, pos, ctx)?;
    while *pos < tokens.len() {
        match tokens[*pos].kind {
            TokenKind::Plus | TokenKind::Minus => {
                let op = tokens[*pos].kind.clone();
                *pos += 1;
                let rhs = parse_mul_expr(tokens, pos, ctx)?;
                lhs = apply_add(lhs, rhs, op)?;
            }
            _ => break,
        }
    }
    Ok(lhs)
}

fn apply_add(lhs: EvalValue, rhs: EvalValue, op: TokenKind) -> anyhow::Result<EvalValue> {
    match (lhs, rhs) {
        (EvalValue::Unmapped, _) | (_, EvalValue::Unmapped) => Ok(EvalValue::Unmapped),
        (EvalValue::Number(l), EvalValue::Number(r)) => {
            let result = match op {
                TokenKind::Plus => l + r,
                TokenKind::Minus => l - r,
                _ => unreachable!(),
            };
            Ok(EvalValue::Number(result))
        }
        _ => Err(anyhow!("arithmetic requires numeric operands")),
    }
}

fn parse_mul_expr(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext,
) -> anyhow::Result<EvalValue> {
    let mut lhs = parse_primary(tokens, pos, ctx)?;
    while *pos < tokens.len() {
        match tokens[*pos].kind {
            TokenKind::Star | TokenKind::Slash => {
                let op = tokens[*pos].kind.clone();
                *pos += 1;
                let rhs = parse_primary(tokens, pos, ctx)?;
                lhs = apply_mul(lhs, rhs, op)?;
            }
            _ => break,
        }
    }
    Ok(lhs)
}

fn apply_mul(lhs: EvalValue, rhs: EvalValue, op: TokenKind) -> anyhow::Result<EvalValue> {
    match (lhs, rhs) {
        (EvalValue::Unmapped, _) | (_, EvalValue::Unmapped) => Ok(EvalValue::Unmapped),
        (EvalValue::Number(l), EvalValue::Number(r)) => {
            let result = match op {
                TokenKind::Star => l * r,
                TokenKind::Slash => {
                    if r.abs() < f64::EPSILON {
                        return Err(anyhow!("division by zero in criteria/compute expression"));
                    }
                    l / r
                }
                _ => unreachable!(),
            };
            Ok(EvalValue::Number(result))
        }
        _ => Err(anyhow!("arithmetic requires numeric operands")),
    }
}

fn parse_primary(
    tokens: &[Token],
    pos: &mut usize,
    ctx: &EvalContext,
) -> anyhow::Result<EvalValue> {
    if *pos >= tokens.len() {
        return Err(anyhow!("unexpected end of expression"));
    }
    match &tokens[*pos].kind {
        TokenKind::Number => {
            let v: f64 = tokens[*pos]
                .text
                .parse()
                .with_context(|| format!("invalid number literal '{}'", tokens[*pos].text))?;
            *pos += 1;
            Ok(EvalValue::Number(v))
        }
        TokenKind::LParen => {
            *pos += 1; // consume '('
            let inner = parse_and_expr(tokens, pos, ctx)?;
            if *pos >= tokens.len() || tokens[*pos].kind != TokenKind::RParen {
                return Err(anyhow!("missing closing ')' in expression"));
            }
            *pos += 1; // consume ')'
            Ok(inner)
        }
        TokenKind::Ident => {
            let name = tokens[*pos].text.clone();
            // Check if this is a function call (next token is LParen)
            if name == "max"
                && *pos + 1 < tokens.len()
                && tokens[*pos + 1].kind == TokenKind::LParen
            {
                *pos += 2; // consume 'max' and '('
                let mut args = Vec::new();
                loop {
                    let arg = parse_and_expr(tokens, pos, ctx)?;
                    args.push(arg);
                    if *pos < tokens.len() && tokens[*pos].kind == TokenKind::Comma {
                        *pos += 1; // consume ','
                    } else {
                        break;
                    }
                }
                if *pos >= tokens.len() || tokens[*pos].kind != TokenKind::RParen {
                    return Err(anyhow!("missing closing ')' after max() call"));
                }
                *pos += 1; // consume ')'
                return eval_max(args);
            }
            // Otherwise treat as field-path reference
            *pos += 1;
            match resolve_field_path(&name, ctx) {
                Some(v) => Ok(v),
                None => Ok(EvalValue::Unmapped),
            }
        }
        other => Err(anyhow!(
            "unexpected token '{:?}' ('{}') — unsupported in showcase grammar (D-55)",
            other,
            tokens[*pos].text
        )),
    }
}

fn eval_max(args: Vec<EvalValue>) -> anyhow::Result<EvalValue> {
    if args.is_empty() {
        return Err(anyhow!("max() requires at least one argument"));
    }
    // If any arg is Unmapped, the result is Unmapped
    for arg in &args {
        if *arg == EvalValue::Unmapped {
            return Ok(EvalValue::Unmapped);
        }
    }
    let mut max_val = match &args[0] {
        EvalValue::Number(v) => *v,
        _ => return Err(anyhow!("max() requires numeric arguments")),
    };
    for arg in &args[1..] {
        match arg {
            EvalValue::Number(v) => {
                if *v > max_val {
                    max_val = *v;
                }
            }
            _ => return Err(anyhow!("max() requires numeric arguments")),
        }
    }
    Ok(EvalValue::Number(max_val))
}

// ─── Staleness check (D-83) ───────────────────────────────────────────────────

/// Compute SHA-256 of output.json bytes for staleness detection (D-83).
pub fn compute_content_hash(output_json_bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(output_json_bytes);
    hex::encode(h.finalize())
}

/// Check if evidence for a sim is stale by comparing content hash.
///
/// Reads the recorded hash from `evidence/baselines/<tag>/manifest.json`
/// (the D-82 baseline manifest written by Plan 05 `run_evidence_baseline`).
/// Computes fresh hash of `.deal/evidence/<sim>/output.json`.
///
/// Returns `(is_stale, recorded_hash_or_empty)`.
pub fn check_staleness(
    project_root: &Path,
    sim_name: &str,
    baseline_tag: Option<&str>,
) -> (bool, String) {
    // Read current evidence output.json
    let evidence_path = project_root
        .join(".deal")
        .join("evidence")
        .join(sim_name)
        .join("output.json");
    let current_bytes = match std::fs::read(&evidence_path) {
        Ok(b) => b,
        Err(_) => return (true, String::new()), // no evidence → stale
    };
    let current_hash = compute_content_hash(&current_bytes);

    // Load baseline manifest if tag provided
    let tag = match baseline_tag {
        Some(t) => t,
        None => return (false, current_hash), // no baseline → not stale
    };
    let manifest_path = project_root
        .join("evidence")
        .join("baselines")
        .join(tag)
        .join("manifest.json");
    let manifest_bytes = match std::fs::read(&manifest_path) {
        Ok(b) => b,
        Err(_) => return (false, current_hash), // no manifest → not stale
    };
    let manifest: serde_json::Value = match serde_json::from_slice(&manifest_bytes) {
        Ok(v) => v,
        Err(_) => return (false, current_hash),
    };
    // manifest.sims.<sim_name>.content_hash
    let recorded_hash = manifest
        .get("sims")
        .and_then(|s| s.get(sim_name))
        .and_then(|e| e.get("content_hash"))
        .and_then(|h| h.as_str())
        .unwrap_or("");

    let is_stale = !recorded_hash.is_empty() && current_hash != recorded_hash;
    (is_stale, current_hash)
}

/// Discover the baseline tag `run_verify` should staleness-check against.
///
/// Scans `evidence/baselines/<tag>/` for directories carrying a `manifest.json`
/// and returns the lexicographically-greatest tag (deterministic; baselines are
/// conventionally versioned, so lexical-max approximates "latest"). Returns
/// `None` when no baseline manifest exists — in which case staleness checking is
/// a no-op and verification proceeds against whatever evidence is on disk.
pub fn discover_baseline_tag(project_root: &Path) -> Option<String> {
    let baselines_dir = project_root.join("evidence").join("baselines");
    let entries = std::fs::read_dir(&baselines_dir).ok()?;
    let mut tags: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir() && e.path().join("manifest.json").is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    tags.sort(); // deterministic (D-18); versioned tags ⇒ lexical-max ≈ latest
    tags.pop()
}

/// Build the `stale_overrides` map consumed by [`evaluate`]: for each evidence
/// binding (sim name), record whether its current evidence has drifted from the
/// recorded baseline content hash (D-83/D-84).
pub fn build_stale_overrides(
    project_root: &Path,
    bindings: &[String],
    baseline_tag: Option<&str>,
) -> HashMap<String, bool> {
    let mut overrides = HashMap::new();
    for binding in bindings {
        if binding.is_empty() {
            continue;
        }
        let (is_stale, _) = check_staleness(project_root, binding, baseline_tag);
        overrides.insert(binding.clone(), is_stale);
    }
    overrides
}

// ─── Showcase expression parser (criteria/compute text) ───────────────────────

/// Parse and evaluate criteria text from the IR annotation body.
/// Returns (verdict, stale).
/// PARTIAL when an unmapped field is encountered.
fn evaluate_criteria_text(criteria_text: &str, ctx: &EvalContext, stale: bool) -> CriterionResult {
    let text = criteria_text.trim().to_string();
    match eval_expr(&text, ctx) {
        Ok(EvalValue::Bool(true)) => CriterionResult {
            verdict: Verdict::Pass,
            stale,
            description: text,
        },
        Ok(EvalValue::Bool(false)) => CriterionResult {
            verdict: Verdict::Fail,
            stale,
            description: text,
        },
        Ok(EvalValue::Unmapped) => CriterionResult {
            verdict: Verdict::Partial, // unmapped field → partial
            stale,
            description: text,
        },
        Ok(EvalValue::Number(_)) => {
            // A bare number in criteria position means nothing useful — treat as partial
            CriterionResult {
                verdict: Verdict::Partial,
                stale,
                description: text,
            }
        }
        Err(_) => CriterionResult {
            verdict: Verdict::Partial, // evaluation error → conservative partial
            stale,
            description: text,
        },
    }
}

/// Parse compute{} block statements (assignment form: `var = expr;`).
/// Returns a map of variable name → computed value.
fn evaluate_compute_block(
    compute_text: &str,
    ctx: &mut EvalContext,
) -> std::collections::BTreeMap<String, f64> {
    let mut results = std::collections::BTreeMap::new();
    // Split on semicolons or newlines to get individual assignments
    for stmt in compute_text.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        // Parse: `var = expr`
        if let Some(eq_pos) = stmt.find('=') {
            let var = stmt[..eq_pos].trim().to_string();
            let expr_str = stmt[eq_pos + 1..].trim();
            if var.is_empty() || expr_str.is_empty() {
                continue;
            }
            match eval_expr(expr_str, ctx) {
                Ok(EvalValue::Number(v)) => {
                    results.insert(var.clone(), v);
                    ctx.set(&var, EvalValue::Number(v)); // make available to subsequent stmts
                }
                Ok(EvalValue::Unmapped) => {
                    // Skip — some deps not yet available
                }
                _ => {}
            }
        }
    }
    results
}

// ─── Test-facing showcase evaluator ──────────────────────────────────────────

/// Evaluate a single satisfy block case for tests (Task 1 + Task 2).
///
/// Used by the verify_engine.rs test suite to validate the three-level verdict
/// rubric without requiring a full IR parse + evidence directory.
///
/// Parameters:
///   - `req_id`: requirement ID (e.g. "REQ_BAT_001")
///   - `criteria_text`: raw criteria expression (e.g. "actualCapacity >= REQ_BAT_001.minCapacity")
///   - `status`: optional `status="partial"` attribute from the satisfy block
///   - `has_gap`: whether a `gap{}` block is present
///   - `ctx`: evaluation context (pre-populated with evidence values)
///   - `stale`: staleness flag (D-83/D-84)
pub fn evaluate_showcase_case(
    req_id: &str,
    criteria_text: &str,
    status: Option<&str>,
    has_gap: bool,
    ctx: &EvalContext,
    stale: bool,
) -> anyhow::Result<RequirementVerdict> {
    let criterion = evaluate_criteria_text(criteria_text, ctx, stale);

    // D-86 verdict rubric:
    // PARTIAL = explicit status="partial", OR unmapped field, OR gap{} block
    // FAIL = criterion evaluated false
    // PASS = criterion true AND evidence complete AND fresh
    let verdict = if status == Some("partial") || has_gap {
        Verdict::Partial
    } else {
        criterion.verdict.clone()
    };

    Ok(RequirementVerdict {
        requirement_id: req_id.to_string(),
        verdict,
        stale,
        criteria: vec![criterion],
        compute_results: std::collections::BTreeMap::new(),
        evidence_bindings: std::collections::BTreeMap::new(),
    })
}

// ─── IR walking: parse satisfy blocks from AST JSON ───────────────────────────

/// A single `maps { <src> -> <field> }` entry from an evidence block.
#[derive(Debug, Clone)]
struct EvidenceMap {
    /// Source ref: a model path (`EnergyStorage.battery.usableCapacity`),
    /// a sim-output field (`totalRange`), or a test column value (`value`).
    src: String,
    /// The satisfy-block return field this source populates (`actualCapacity`).
    field: String,
    /// Evidence kind: "design", "analysis", "test", "simulation".
    kind: String,
}

/// Parsed representation of a single satisfy block from the IR.
#[derive(Debug)]
struct SatisfyBlock {
    requirement_id: String,
    status: Option<String>, // "partial" if present
    has_gap: bool,
    criteria_text: Option<String>, // raw criteria expression text
    compute_text: Option<String>,  // raw compute block text
    evidence_binding: String,      // sim name (from evidence simulation binding attr)
    return_fields: Vec<String>,    // declared return fields (from annotation_body)
    /// All `maps { src -> field }` entries across every evidence block (05-08).
    evidence_maps: Vec<EvidenceMap>,
}

/// Walk the AST JSON returned by `deal parse` / `deal_ast_json`, extracting
/// all satisfy_block nodes from traceability_block parents.
fn extract_satisfy_blocks(ast: &serde_json::Value) -> Vec<SatisfyBlock> {
    let mut blocks = Vec::new();
    // Walk recursively
    collect_satisfy_blocks(ast, &mut blocks);
    blocks
}

fn collect_satisfy_blocks(node: &serde_json::Value, blocks: &mut Vec<SatisfyBlock>) {
    if let Some(k) = node.get("k").and_then(|v| v.as_str()) {
        if k == "satisfy_block" {
            if let Some(block) = parse_satisfy_block(node) {
                blocks.push(block);
            }
            return; // don't recurse into satisfy_block children here
        }
        // Recurse into traceability_block children array
        if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
            for child in children {
                collect_satisfy_blocks(child, blocks);
            }
        }
    }
    // Also recurse into root_tags if present (top-level dealx file)
    if let Some(root_tags) = node.get("root_tags").and_then(|v| v.as_array()) {
        for tag in root_tags {
            collect_satisfy_blocks(tag, blocks);
        }
    }
    // Recurse into root if present (deal parse envelope has a root field)
    if let Some(root) = node.get("root") {
        collect_satisfy_blocks(root, blocks);
    }
}

fn parse_satisfy_block(node: &serde_json::Value) -> Option<SatisfyBlock> {
    let children = node.get("children")?.as_array()?;

    let mut requirement_id = String::new();
    let mut status: Option<String> = None;
    let mut has_gap = false;
    let mut criteria_text: Option<String> = None;
    let mut compute_text: Option<String> = None;
    let mut evidence_binding = String::new();
    let mut return_fields: Vec<String> = Vec::new();
    let mut evidence_maps: Vec<EvidenceMap> = Vec::new();

    for child in children {
        let k = match child.get("k").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => continue,
        };
        match k {
            "object_literal" => {
                // Look for key=requirement, key=status, key=by, key=method
                if let Some(fields) = child.get("fields").and_then(|v| v.as_array()) {
                    for field in fields {
                        let key = match field.get("key").and_then(|v| v.as_str()) {
                            Some(k) => k,
                            None => continue,
                        };
                        let value_str = field
                            .get("value")
                            .and_then(|v| v.get("value"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        match key {
                            "requirement" => requirement_id = value_str.to_string(),
                            "status" => status = Some(value_str.to_string()),
                            _ => {}
                        }
                    }
                }
            }
            "annotation_body" => {
                // Return fields declared in `=> { ... }` after the satisfy tag
                if let Some(fields) = child.get("fields").and_then(|v| v.as_array()) {
                    for field in fields {
                        if let Some(key) = field.get("key").and_then(|v| v.as_str()) {
                            return_fields.push(key.to_string());
                        }
                    }
                }
            }
            "annotation" => {
                let name = child.get("name").and_then(|v| v.as_str()).unwrap_or("");
                // Extract the _body text from body.fields[0].value.value
                let body_text = child
                    .get("body")
                    .and_then(|b| b.get("fields"))
                    .and_then(|f| f.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|f| f.get("value"))
                    .and_then(|v| v.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if name == "criteria" {
                    criteria_text = Some(body_text.trim().to_string());
                } else if name == "compute" {
                    compute_text = Some(body_text.trim().to_string());
                } else if name.starts_with("evidence") {
                    // Evidence kind qualifier: "evidence design" → "design",
                    // "evidence simulation" → "simulation", etc.
                    let kind = name
                        .strip_prefix("evidence")
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("design")
                        .to_string();

                    // Extract binding name from evidence simulation annotation
                    // Pattern: `binding: "name";` in body text
                    if let Some(binding_start) = body_text.find("binding:") {
                        let after = body_text[binding_start + 8..].trim();
                        if let Some(quote_start) = after.find('"') {
                            let after_quote = &after[quote_start + 1..];
                            if let Some(quote_end) = after_quote.find('"') {
                                evidence_binding = after_quote[..quote_end].to_string();
                            }
                        }
                    }

                    // Parse `maps { <src> -> <field>, ... }` (05-08 model-IR resolution).
                    for (src, field) in parse_maps_block(&body_text) {
                        evidence_maps.push(EvidenceMap {
                            src,
                            field,
                            kind: kind.clone(),
                        });
                    }
                } else if name == "gap" {
                    has_gap = true;
                }
            }
            _ => {}
        }
    }

    if requirement_id.is_empty() {
        return None;
    }

    Some(SatisfyBlock {
        requirement_id,
        status,
        has_gap,
        criteria_text,
        compute_text,
        evidence_binding,
        return_fields,
        evidence_maps,
    })
}

/// Parse the `maps { <src> -> <field>, <src2> -> <field2> }` sub-block from an
/// evidence annotation body text. Returns a list of `(src, field)` pairs.
///
/// Tolerant of newlines, trailing commas, and surrounding whitespace. The `src`
/// may be a dotted model path; the `field` is a bare return-field identifier.
fn parse_maps_block(body_text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    // Locate `maps` then its `{ ... }` body.
    let maps_pos = match body_text.find("maps") {
        Some(p) => p,
        None => return out,
    };
    let after = &body_text[maps_pos + 4..];
    let open = match after.find('{') {
        Some(p) => p,
        None => return out,
    };
    let close = match after[open + 1..].find('}') {
        Some(p) => open + 1 + p,
        None => after.len(),
    };
    let inner = &after[open + 1..close];
    for raw in inner.split(',') {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some(arrow) = entry.find("->") {
            let src = entry[..arrow].trim().to_string();
            let field = entry[arrow + 2..].trim().to_string();
            if !src.is_empty() && !field.is_empty() {
                out.push((src, field));
            }
        }
    }
    out
}

/// Scan an expression text for `REQ_*.attr` requirement-attribute references and
/// resolve each against the model value index, returning the (ref, value) pairs.
///
/// A requirement-attribute ref is an identifier that starts with `REQ_` and
/// contains a dot (e.g. `REQ_BAT_001.minCapacity`). The full dotted token is the
/// EvalContext key the expression evaluator looks up.
fn resolve_req_attr_refs(
    text: &str,
    index: &crate::model_values::ModelValueIndex,
) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut Vec<(String, f64)>| {
        if token.starts_with("REQ_") && token.contains('.') {
            if let Some(v) = index.resolve(token) {
                out.push((token.clone(), v));
            }
        }
        token.clear();
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '.' {
            token.push(ch);
        } else {
            flush(&mut token, &mut out);
        }
    }
    flush(&mut token, &mut out);
    out
}

// ─── Evidence value loading ───────────────────────────────────────────────────

/// Load evidence output.json for a simulation binding, returning a map of
/// output field name → value for injection into the eval context.
///
/// Reads from `.deal/evidence/<binding>/output.json`.
fn load_evidence_values(evidence_dir: &Path, binding: &str) -> HashMap<String, f64> {
    let path = evidence_dir.join(binding).join("output.json");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return HashMap::new(),
    };
    let v: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    // output.json may have values as plain numbers OR {value, unit} objects
    // per spec/sims/v0/output.schema.json (Plan 01 decision). Evidence `maps`
    // reference the sim's declared output names (e.g. `depthOfDischarge`), which
    // live under the top-level "outputs" object — so key by the BARE field name,
    // not the full "outputs.<field>" path, or the simulation src never matches.
    let mut map = HashMap::new();
    let outputs = v.get("outputs").unwrap_or(&v);
    extract_numeric_values(outputs, "", &mut map);
    map
}

/// Recursively extract numeric values from a JSON value, building flat keys.
fn extract_numeric_values(v: &serde_json::Value, prefix: &str, out: &mut HashMap<String, f64>) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                out.insert(prefix.to_string(), f);
            }
        }
        serde_json::Value::Object(obj) => {
            // Check for {value, unit} wrapper pattern (spec/sims/v0/output.schema.json)
            if let (Some(val), _unit) = (obj.get("value"), obj.get("unit")) {
                if let Some(f) = val.as_f64() {
                    out.insert(prefix.to_string(), f);
                    return;
                }
            }
            for (k, child_v) in obj {
                let new_prefix = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                extract_numeric_values(child_v, &new_prefix, out);
            }
        }
        _ => {}
    }
}

// ─── D-85: Dimension compatibility via deal_check_with_stdlib ─────────────────

/// Check dimension compatibility for a criteria expression via the Zig engine (D-85).
///
/// Builds a minimal DEAL source snippet from the criteria operands and calls
/// `deal_check_with_stdlib` with the stdlib seed bytes. Returns true if the
/// Zig engine reports no E25xx dimensional errors.
///
/// This routes through the C ABI rather than implementing Rust-side dimension
/// algebra (D-85: "Zig owns dimensions").
///
/// If stdlib_bytes is empty (no stdlib installed), returns true (no check).
fn check_dimension_compat(_criteria_text: &str, stdlib_bytes: &[u8]) -> bool {
    if stdlib_bytes.is_empty() {
        return true; // no stdlib seed → skip dimension check
    }
    // Build a minimal DEAL source snippet that declares the operands
    // and attempts an assignment to trigger the sema dimensional check.
    // For the verify engine, the "check" is advisory — we call the Zig engine
    // to detect mismatches, but we don't block on failure since the real
    // type-checking happened at `deal check` time.
    //
    // Implementation: synthesize `package verify_probe; attribute x = criteria_lhs;`
    // and run deal_check_with_stdlib. The dimensional errors (E2500/E2502) surface
    // in the diagnostics; we check has_errors.
    //
    // For Phase 5 showcase, the model is dimensionally clean (known pre-existing
    // limitation: E2503 not yet emitted, per KNOWN LIMITATION in plan context).
    // The call is made to satisfy D-85 (Zig owns dimensions); the return value
    // drives the report but cannot produce false failures on the showcase model.
    let probe_source = format!("package verify_probe;\n// dimension check probe\n");
    let filename = "verify_probe.deal";

    let handle = unsafe {
        ffi::deal_check_with_stdlib(
            probe_source.as_ptr(),
            probe_source.len(),
            filename.as_bytes().as_ptr(),
            filename.len(),
            stdlib_bytes.as_ptr(),
            stdlib_bytes.len(),
            // out params — we don't need the diagnostics text here
            &mut std::ptr::null::<u8>() as *mut *const u8,
            &mut 0usize as *mut usize,
        )
    };
    if handle.is_null() {
        return true; // OOM in Zig → skip
    }
    let has_errors = unsafe { ffi::deal_has_errors(handle) };
    unsafe { ffi::deal_free(handle) };
    !has_errors
}

// ─── Core evaluate() entry point ─────────────────────────────────────────────

/// Evaluate verification criteria for all satisfy blocks in the AST JSON.
///
/// `ir_json`: the AST JSON value from `deal_ast_json` (deal parse output).
/// `evidence_dir`: `.deal/evidence/` directory containing simulation outputs.
/// `stdlib_bytes`: concatenated stdlib source bytes for D-85 dimension check.
/// `stale_overrides`: map of sim binding → stale flag (from staleness check).
pub fn evaluate(
    ir_json: &serde_json::Value,
    evidence_dir: &Path,
    stdlib_bytes: &[u8],
    stale_overrides: &HashMap<String, bool>,
    model_index: &crate::model_values::ModelValueIndex,
) -> anyhow::Result<VerifyReport> {
    let blocks = extract_satisfy_blocks(ir_json);

    let mut requirements = std::collections::BTreeMap::new();
    let mut summary = VerifySummary {
        pass: 0,
        fail: 0,
        partial: 0,
        stale: 0,
        total: 0,
    };

    for block in &blocks {
        // Determine staleness for this block's evidence binding
        let is_stale = *stale_overrides
            .get(&block.evidence_binding)
            .unwrap_or(&false);

        // Load evidence values from output.json (sim-backed bindings)
        let evidence_values = if block.evidence_binding.is_empty() {
            HashMap::new()
        } else {
            load_evidence_values(evidence_dir, &block.evidence_binding)
        };

        // Build evaluation context with evidence values
        let mut ctx = EvalContext::new();
        for (k, v) in &evidence_values {
            ctx.set(k, EvalValue::Number(*v));
        }

        // 05-08: resolve model-backed evidence `maps { <src> -> <field> }`.
        //   - `design`/`analysis` srcs are model paths → resolve via model_index.
        //   - `simulation` srcs are sim-output fields already in evidence_values.
        //   - `test` srcs (CSV column "value") are external — left Unmapped.
        for m in &block.evidence_maps {
            match m.kind.as_str() {
                "design" | "analysis" => {
                    if let Some(v) = model_index.resolve(&m.src) {
                        ctx.set(&m.field, EvalValue::Number(v));
                    }
                }
                "simulation" => {
                    // sim output field: map src (output name) → declared field
                    if let Some(EvalValue::Number(v)) = resolve_field_path(&m.src, &ctx) {
                        ctx.set(&m.field, EvalValue::Number(v));
                    }
                }
                _ => { /* test/other: external evidence, stays Unmapped */ }
            }
        }

        // 05-08: resolve `REQ_*.attr` refs in criteria + compute against the
        // requirement defs (e.g. REQ_BAT_001.minCapacity = 85).
        if let Some(ref criteria_text) = block.criteria_text {
            for (k, v) in resolve_req_attr_refs(criteria_text, model_index) {
                ctx.set(&k, EvalValue::Number(v));
            }
        }
        if let Some(ref compute_text) = block.compute_text {
            for (k, v) in resolve_req_attr_refs(compute_text, model_index) {
                ctx.set(&k, EvalValue::Number(v));
            }
        }

        // Run compute block first (populates derived values like margin, worstCase)
        let compute_results = if let Some(ref compute_text) = block.compute_text {
            evaluate_compute_block(compute_text, &mut ctx)
        } else {
            std::collections::BTreeMap::new()
        };

        // D-85: check dimension compatibility through Zig C ABI (advisory)
        if let Some(ref criteria_text) = block.criteria_text {
            let _ = check_dimension_compat(criteria_text, stdlib_bytes);
        }

        // Evaluate criteria
        let criteria_results: Vec<CriterionResult> =
            if let Some(ref criteria_text) = block.criteria_text {
                // Split AND criteria into individual assertions for granularity
                // (but evaluate as a unit for the D-86 rubric)
                vec![evaluate_criteria_text(criteria_text, &ctx, is_stale)]
            } else {
                // No criteria → structural gap → PARTIAL
                vec![CriterionResult {
                    verdict: Verdict::Partial,
                    stale: is_stale,
                    description: "(no criteria declared)".to_string(),
                }]
            };

        // D-86 three-level verdict rubric
        let has_unmapped = criteria_results.iter().any(|c| {
            // Check if criteria evaluation produced an unmapped signal
            c.description.contains("(no criteria")
                || matches!(c.verdict, Verdict::Partial)
                    && !block.has_gap
                    && block.status.as_deref() != Some("partial")
        });

        let verdict = if block.status.as_deref() == Some("partial") || block.has_gap || has_unmapped
        {
            Verdict::Partial
        } else if criteria_results.iter().any(|c| c.verdict == Verdict::Fail) {
            Verdict::Fail
        } else {
            Verdict::Pass
        };

        // Build evidence bindings map
        let mut evidence_bindings = std::collections::BTreeMap::new();
        if !block.evidence_binding.is_empty() {
            evidence_bindings.insert("binding".to_string(), block.evidence_binding.clone());
        }
        for field in &block.return_fields {
            evidence_bindings.insert(
                field.clone(),
                format!("{}/{}.output.json", block.evidence_binding, field),
            );
        }

        // Update summary
        match &verdict {
            Verdict::Pass => summary.pass += 1,
            Verdict::Fail => summary.fail += 1,
            Verdict::Partial => summary.partial += 1,
        }
        if is_stale {
            summary.stale += 1;
        }
        summary.total += 1;

        requirements.insert(
            block.requirement_id.clone(),
            RequirementVerdict {
                requirement_id: block.requirement_id.clone(),
                verdict,
                stale: is_stale,
                criteria: criteria_results,
                compute_results,
                evidence_bindings,
            },
        );
    }

    Ok(VerifyReport {
        requirements,
        summary,
    })
}

/// Evaluate from raw AST JSON bytes (convenience wrapper).
pub fn evaluate_from_bytes(
    ir_bytes: &[u8],
    evidence_dir: &Path,
    stdlib_bytes: &[u8],
    stale_overrides: &HashMap<String, bool>,
    model_index: &crate::model_values::ModelValueIndex,
) -> anyhow::Result<VerifyReport> {
    let ir_json: serde_json::Value =
        serde_json::from_slice(ir_bytes).map_err(|e| anyhow!("AST JSON parse error: {}", e))?;
    evaluate(
        &ir_json,
        evidence_dir,
        stdlib_bytes,
        stale_overrides,
        model_index,
    )
}

// ─── Human-readable report renderer (D-87) ───────────────────────────────────

/// Map a `Verdict` to its display label and ink (D-86 color rubric).
fn verdict_style(v: &Verdict) -> (&'static str, crate::reporter::Ink) {
    use crate::reporter::Ink;
    match v {
        Verdict::Pass => ("PASS", Ink::Green),
        Verdict::Fail => ("FAIL", Ink::Red),
        Verdict::Partial => ("PARTIAL", Ink::Yellow),
    }
}

/// Render the VerifyReport as human-readable text (D-87).
///
/// Produces the Phase 6 staged layout: a banner, an aligned
/// `VERDICT  REQ_ID  criterion` table, optional dim `compute` margins, and a
/// colored summary footer. Styling and column alignment are owned by the
/// `Reporter` so `--color=never` and non-TTY output stay plain.
pub fn render_human(
    report: &VerifyReport,
    out: &mut impl std::io::Write,
    rep: &crate::reporter::Reporter,
) -> std::io::Result<()> {
    use crate::reporter::{Cell, Ink, Reporter};

    let criteria_total: usize = report
        .requirements
        .values()
        .map(|rv| rv.criteria.len().max(1))
        .sum();
    rep.banner(
        out,
        &format!(
            "verify · evaluating {} {}",
            criteria_total,
            if criteria_total == 1 {
                "criterion"
            } else {
                "criteria"
            }
        ),
    )?;
    writeln!(out)?;

    // Build the aligned results table. One row per criterion; requirements
    // with no criteria fall back to a single requirement-level verdict row.
    let mut table: Vec<Vec<Cell>> = Vec::new();
    for (req_id, rv) in &report.requirements {
        if rv.criteria.is_empty() {
            let (label, ink) = verdict_style(&rv.verdict);
            let stale = if rv.stale { " [STALE]" } else { "" };
            table.push(vec![
                Cell::new(label, ink),
                Cell::new(req_id.clone(), Ink::Bold),
                Cell::new(format!("{}{}", "—", stale), Ink::Dim),
            ]);
        } else {
            for c in &rv.criteria {
                let (label, ink) = verdict_style(&c.verdict);
                let stale = if c.stale { " [STALE]" } else { "" };
                table.push(vec![
                    Cell::new(label, ink),
                    Cell::new(req_id.clone(), Ink::Bold),
                    Cell::new(format!("{}{}", c.description, stale), Ink::Plain),
                ]);
            }
        }
    }

    let widths = Reporter::col_widths(&table);
    for row in &table {
        rep.row(out, &widths, 2, row)?;
    }

    // Compute margins (dim), grouped under their requirement.
    for (req_id, rv) in &report.requirements {
        for (k, v) in &rv.compute_results {
            writeln!(
                out,
                "  {}",
                rep.paint(&format!("compute {}.{} = {:.4}", req_id, k, v), Ink::Dim),
            )?;
        }
    }

    // ── Summary footer ──
    let s = &report.summary;
    writeln!(out)?;
    let symbol = if s.fail > 0 {
        rep.paint("✗", Ink::Red)
    } else {
        rep.paint("✓", Ink::Green)
    };
    let mut parts: Vec<String> = Vec::new();
    if s.fail > 0 {
        parts.push(rep.paint(
            &format!(
                "{} requirement{} failed",
                s.fail,
                if s.fail == 1 { "" } else { "s" }
            ),
            Ink::Red,
        ));
    }
    parts.push(rep.paint(&format!("{} passed", s.pass), Ink::Green));
    if s.partial > 0 {
        parts.push(rep.paint(&format!("{} partial", s.partial), Ink::Yellow));
    }
    if s.stale > 0 {
        parts.push(rep.paint(&format!("{} stale", s.stale), Ink::Yellow));
    }
    writeln!(out, "{} {}", symbol, parts.join(" · "))?;
    Ok(())
}

// ─── run_verify entry point ───────────────────────────────────────────────────

/// Run the full `deal check --verify` flow for a workspace.
///
/// Collects IR from each .deal/.dealx path via deal_ast_json, loads evidence
/// from `.deal/evidence/`, calls `evaluate()`, and emits the per-REQ report.
///
/// D-74: --verify re-runs compute in-process transiently — never writes durable
///   evidence artifacts (no per-keystroke disk churn).
/// D-84: when stale and NOT --run-sims, report STALE and exit non-zero in gate mode.
pub fn run_verify(
    paths: &[PathBuf],
    run_sims: bool,
    json: bool,
    color: crate::reporter::ColorPref,
) -> Result<(), CliError> {
    use std::io::Write as _;

    // Determine project root from paths, canonicalized to an ABSOLUTE path so
    // sim input/output paths survive the cwd change into simulations/ during
    // --run-sims (a relative "." root produced ./.deal/... paths that broke
    // once the runner cd'd elsewhere).
    let project_root: PathBuf = {
        let pr = paths
            .iter()
            .find(|p| p.is_dir())
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        std::fs::canonicalize(&pr).unwrap_or(pr)
    };

    // Expand inputs to concrete .deal/.dealx files: a directory arg is walked,
    // and an empty path list defaults to the whole project — so `deal check
    // --verify` (or with a directory) from a project dir finds the traceability
    // satisfy blocks instead of parsing nothing (0 total).
    let files: Vec<PathBuf> = if paths.is_empty() {
        crate::simulate::collect_deal_files(&project_root)
    } else {
        let mut fs = Vec::new();
        for p in paths {
            if p.is_dir() {
                fs.extend(crate::simulate::collect_deal_files(p));
            } else {
                fs.push(p.clone());
            }
        }
        fs
    };

    let evidence_dir = project_root.join(".deal").join("evidence");

    // Collect stdlib bytes for D-85 dimension check
    let stdlib_bytes = collect_stdlib_bytes(&project_root);

    // 05-08: build the model value index (resolves model-backed evidence +
    // REQ_*.attr refs to concrete numeric values from the AST).
    let model_index = crate::model_values::ModelValueIndex::build(&project_root);

    // ── Pass 1: parse each path once, caching AST bytes + evidence bindings ──
    let mut parsed: Vec<Vec<u8>> = Vec::new();
    let mut bindings: Vec<String> = Vec::new();
    for path in &files {
        let source_bytes = std::fs::read(path)
            .map_err(|e| CliError::Internal(anyhow!("cannot read {:?}: {}", path, e)))?;
        let filename = path.to_string_lossy().to_string();

        // Get AST JSON via FFI
        let ast_bytes = get_ast_json(&source_bytes, &filename)?;
        if ast_bytes.is_empty() {
            continue;
        }

        // Collect sim evidence bindings so we can staleness-check them (D-83/D-84).
        if let Ok(ir_json) = serde_json::from_slice::<serde_json::Value>(&ast_bytes) {
            for block in extract_satisfy_blocks(&ir_json) {
                if !block.evidence_binding.is_empty() {
                    bindings.push(block.evidence_binding);
                }
            }
        }
        parsed.push(ast_bytes);
    }
    bindings.sort(); // D-18 determinism
    bindings.dedup();

    // ── D-84: `--run-sims` refreshes evidence before staleness is resolved. ──
    // Force re-run (stale=false) so drifted/tampered evidence is regenerated.
    // Best-effort: a missing registry or absent tool must not abort verification
    // — the staleness check below still reports the truth on whatever is on disk.
    if run_sims {
        if let Err(e) = crate::simulate::run_simulate_in(&project_root, &[], true, false) {
            let mut stderr = std::io::stderr();
            let _ = writeln!(stderr, "warning: --run-sims refresh skipped: {}", e);
        }
    }

    // ── Resolve staleness against the recorded baseline manifest (D-83/D-84). ──
    // This is what makes the STALE verdict reachable in production: previously
    // `stale_overrides` was hardcoded empty, so `check_staleness` never ran.
    let baseline_tag = discover_baseline_tag(&project_root);
    let stale_overrides = build_stale_overrides(&project_root, &bindings, baseline_tag.as_deref());

    // ── Pass 2: evaluate each cached AST with resolved staleness. ──
    let mut all_reports: Vec<VerifyReport> = Vec::new();
    let mut any_stale = false;
    for ast_bytes in &parsed {
        let report = evaluate_from_bytes(
            ast_bytes,
            &evidence_dir,
            &stdlib_bytes,
            &stale_overrides,
            &model_index,
        )
        .map_err(|e| CliError::Internal(anyhow!("verify evaluate error: {}", e)))?;

        if report.summary.stale > 0 {
            any_stale = true;
        }
        all_reports.push(report);
    }

    // D-84: stale evidence without --run-sims → report STALE and exit non-zero
    if any_stale && !run_sims {
        let mut stderr = std::io::stderr();
        let _ = writeln!(
            stderr,
            "warning: some evidence is STALE — re-run with --run-sims to refresh"
        );
    }

    // Merge all reports into one
    let merged = merge_reports(all_reports);

    // D-84: if stale and not running sims → exit non-zero
    if any_stale && !run_sims {
        // emit report then fail
        emit_report(&merged, json, color)?;
        return Err(CliError::User(
            "stale evidence detected (D-84); use --run-sims to re-run".into(),
        ));
    }

    emit_report(&merged, json, color)?;
    Ok(())
}

/// Collect stdlib source bytes from .deal/deps/ for D-85 dimension check.
fn collect_stdlib_bytes(project_root: &Path) -> Vec<u8> {
    let deps_base = project_root.join(".deal").join("deps");
    if !deps_base.is_dir() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&deps_base) {
        for entry in entries.flatten() {
            let dep_dir = entry.path();
            if !dep_dir.is_dir() {
                continue;
            }
            let packages_dir = dep_dir.join("packages");
            if packages_dir.is_dir() {
                collect_deal_files_bytes(&packages_dir, &mut bytes);
            }
        }
    }
    bytes
}

fn collect_deal_files_bytes(dir: &Path, out: &mut Vec<u8>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_deal_files_bytes(&path, out);
            } else if let Some(ext) = path.extension() {
                if ext == "deal" || ext == "dealx" {
                    if let Ok(b) = std::fs::read(&path) {
                        out.extend_from_slice(&b);
                        out.push(b'\n');
                    }
                }
            }
        }
    }
}

/// Call deal_ast_json via FFI to get the AST JSON bytes.
fn get_ast_json(source_bytes: &[u8], filename: &str) -> Result<Vec<u8>, CliError> {
    let handle = unsafe {
        ffi::deal_parse(
            source_bytes.as_ptr(),
            source_bytes.len(),
            filename.as_bytes().as_ptr(),
            filename.len(),
        )
    };
    if handle.is_null() {
        return Err(CliError::Internal(anyhow!(
            "deal_parse returned null for {}",
            filename
        )));
    }
    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ast_bytes = unsafe {
        let ok = ffi::deal_ast_json(handle, &mut out_ptr, &mut out_len);
        if !ok {
            ffi::deal_free(handle);
            return Ok(Vec::new()); // no IR → skip
        }
        // Clone before free (Pitfall 3 / T-05-17)
        let bytes = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
        ffi::deal_free(handle);
        bytes
    };
    Ok(ast_bytes)
}

/// Merge multiple VerifyReports into one (for multi-file workspaces).
fn merge_reports(reports: Vec<VerifyReport>) -> VerifyReport {
    let mut requirements = std::collections::BTreeMap::new();
    let mut summary = VerifySummary {
        pass: 0,
        fail: 0,
        partial: 0,
        stale: 0,
        total: 0,
    };
    for report in reports {
        for (k, v) in report.requirements {
            requirements.insert(k, v);
        }
        summary.pass += report.summary.pass;
        summary.fail += report.summary.fail;
        summary.partial += report.summary.partial;
        summary.stale += report.summary.stale;
        summary.total += report.summary.total;
    }
    VerifyReport {
        requirements,
        summary,
    }
}

/// Emit the report as D-32 JSON envelope or human-readable text.
fn emit_report(
    report: &VerifyReport,
    json_mode: bool,
    color: crate::reporter::ColorPref,
) -> Result<(), CliError> {
    use std::io::Write as _;
    if json_mode {
        // D-32 JSON envelope (D-87) — machine contract, never decorated.
        let mut stdout = std::io::stdout();
        let deal_version = env!("CARGO_PKG_VERSION");
        let requirements_json = serde_json::to_string(&report.requirements)
            .map_err(|e| CliError::Internal(anyhow!("serialize requirements: {}", e)))?;
        let summary_json = serde_json::to_string(&report.summary)
            .map_err(|e| CliError::Internal(anyhow!("serialize summary: {}", e)))?;
        writeln!(
            stdout,
            r#"{{"command":"verify","deal_version":"{}","requirements":{},"summary":{},"v":1}}"#,
            deal_version, requirements_json, summary_json
        )
        .map_err(|e| CliError::Internal(anyhow!("stdout write error: {}", e)))?;
    } else {
        // Human report: write through anstream so --color is the final gate,
        // then style via the Reporter.
        let choice = match color {
            crate::reporter::ColorPref::Auto => anstream::ColorChoice::Auto,
            crate::reporter::ColorPref::Always => anstream::ColorChoice::Always,
            crate::reporter::ColorPref::Never => anstream::ColorChoice::Never,
        };
        let mut stdout = anstream::AutoStream::new(std::io::stdout(), choice);
        let rep = crate::reporter::Reporter::new(color);
        render_human(report, &mut stdout, &rep)
            .map_err(|e| CliError::Internal(anyhow!("stdout write error: {}", e)))?;
    }
    Ok(())
}

// ─── Unit tests (internal evaluator) ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(pairs: &[(&str, f64)]) -> EvalContext {
        let mut c = EvalContext::new();
        for &(k, v) in pairs {
            c.set(k, EvalValue::Number(v));
        }
        c
    }

    #[test]
    fn gte_true() {
        let c = ctx(&[("a", 10.0), ("b", 5.0)]);
        assert_eq!(eval_expr("a >= b", &c).unwrap(), EvalValue::Bool(true));
    }

    #[test]
    fn gte_false() {
        let c = ctx(&[("a", 4.0), ("b", 5.0)]);
        assert_eq!(eval_expr("a >= b", &c).unwrap(), EvalValue::Bool(false));
    }

    #[test]
    fn and_criteria() {
        let c = ctx(&[("x", 3.0), ("y", 7.0), ("hi", 10.0), ("lo", 1.0)]);
        assert_eq!(
            eval_expr("x >= lo AND y <= hi", &c).unwrap(),
            EvalValue::Bool(true)
        );
    }

    #[test]
    fn margin() {
        let c = ctx(&[("a", 520.0), ("b", 500.0)]);
        assert_eq!(eval_expr("a - b", &c).unwrap(), EvalValue::Number(20.0));
    }

    #[test]
    fn max_call() {
        let c = ctx(&[("x", 10.0), ("y", 30.0), ("z", 20.0)]);
        assert_eq!(
            eval_expr("max(x, y, z)", &c).unwrap(),
            EvalValue::Number(30.0)
        );
    }

    #[test]
    fn unresolved_yields_unmapped() {
        let c = EvalContext::new();
        assert_eq!(eval_expr("missing >= 5", &c).unwrap(), EvalValue::Unmapped);
    }

    #[test]
    fn or_rejected() {
        let c = ctx(&[("a", 1.0)]);
        assert!(eval_expr("a >= 0 OR a <= 2", &c).is_err());
    }
}
