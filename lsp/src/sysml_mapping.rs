//! DEAL node-kind → SysML v2 / KerML mapping (P1 WS-B).
//!
//! The authoritative table lives in the spec source of truth and is vendored
//! into this build via the `deal/spec` submodule. We embed it at compile time
//! so a malformed table fails the build, never a user's hover request.
//!
//! Consumed by `hover.rs` to render the "SysML v2 · `<metaclass>` (<clause>)"
//! block. Keyed by the AST `k` node-kind string the hover already reads.
//!
//! Provenance: behavioral and expression rows are wiki-verified against the
//! locked `spec/ir/v0.1/behavioral-mapping.md` / `v0.2/expression-mapping.md`
//! contracts; structural metaclass clauses were confirmed via the
//! `sysml-v2-wiki` / `kerml-wiki` closure tool. See `spec/ir/sysml-mapping.json`.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

/// The embedded mapping JSON. Path is relative to this source file:
/// `deal/lsp/src` → `deal/lsp` → `deal` → `deal/spec/ir/...`.
const RAW: &str = include_str!("../../spec/ir/sysml-mapping.json");

/// One resolved mapping row.
#[derive(Debug, Clone)]
pub struct Mapping {
    /// Target SysML v2 / KerML metaclass, e.g. `PartDefinition`.
    pub metaclass: String,
    /// Governing clause, e.g. `SysML 8.3.11.2` or `KerML 8.3.4.7.4`.
    pub clause: String,
    /// KerML basis (specialization terminal), e.g. `Structure`.
    pub kerml: String,
    /// Control nodes that the construct injects as a pair, e.g.
    /// `MergeNode (8.3.17.13)` for `decide`. `None` for most kinds.
    pub injects: Option<String>,
    /// Metadata marker, e.g. `«actor» MetadataFeature (8.3.4.12.3)`.
    pub marker: Option<String>,
    /// Sub-kind variants, e.g. `literal_expr`'s `real` → `LiteralRational
    /// (8.3.4.8.13)`. Consumed by `resolve` to refine expression mappings.
    pub variants: HashMap<String, String>,
}

static TABLE: OnceLock<HashMap<String, Mapping>> = OnceLock::new();

fn table() -> &'static HashMap<String, Mapping> {
    TABLE.get_or_init(|| {
        let root: Value =
            serde_json::from_str(RAW).expect("spec/ir/sysml-mapping.json must be valid JSON");
        let mut out = HashMap::new();
        if let Some(kinds) = root.get("kinds").and_then(|v| v.as_object()) {
            for (kind, v) in kinds {
                let s = |key: &str| v.get(key).and_then(|x| x.as_str()).map(|x| x.to_string());
                // metaclass + clause are required; rows lacking them are skipped.
                if let (Some(metaclass), Some(clause)) = (s("metaclass"), s("clause")) {
                    let variants = v
                        .get("variants")
                        .and_then(|x| x.as_object())
                        .map(|o| {
                            o.iter()
                                .filter_map(|(k, val)| {
                                    val.as_str().map(|s| (k.clone(), s.to_string()))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    out.insert(
                        kind.clone(),
                        Mapping {
                            metaclass,
                            clause,
                            kerml: s("kerml").unwrap_or_default(),
                            injects: s("injects"),
                            marker: s("marker"),
                            variants,
                        },
                    );
                }
            }
        }
        out
    })
}

/// Look up the SysML mapping for an AST/IR node kind. Returns `None` for kinds
/// with no recorded mapping (the hover then omits the SysML block — never an
/// error).
pub fn mapping_for(kind: &str) -> Option<&'static Mapping> {
    table().get(kind)
}

/// Map an AST *expression* node kind to its IR mapping key and an optional
/// `variants` selector. The mapping table is keyed by IR expression names
/// (`operator_expr`, `literal_expr`, …); the AST emits `binary`/`call`/
/// `int_literal`/… The spec rows stay the single source of truth — this only
/// records which IR row (and literal variant) an AST kind corresponds to.
fn normalize_expr_kind(kind: &str) -> Option<(&'static str, Option<&'static str>)> {
    Some(match kind {
        "binary" | "unary" => ("operator_expr", None),
        "call" => ("invocation_expr", None),
        "identifier" => ("feature_ref_expr", None),
        "member_access" => ("feature_ref_expr", Some("dotted_path")),
        "int_literal" => ("literal_expr", None),
        "real_literal" => ("literal_expr", Some("real")),
        "string_literal" | "template_literal" => ("literal_expr", Some("string")),
        "boolean_literal" => ("literal_expr", Some("boolean")),
        _ => return None,
    })
}

/// Split a variant value like `LiteralRational (8.3.4.8.13)` into
/// `(metaclass, clause)`. Falls back to the whole string as the metaclass when
/// it has no parenthesised clause.
fn split_variant(v: &str) -> (String, String) {
    match v.split_once(" (") {
        Some((mc, rest)) => (mc.to_string(), rest.trim_end_matches(')').to_string()),
        None => (v.to_string(), String::new()),
    }
}

/// Resolve a node kind to its SysML mapping, normalizing AST expression kinds
/// to their IR rows and applying the literal/dotted variant where one applies.
/// Returns an owned `Mapping` so callers see a uniform shape regardless of
/// whether a variant override was applied.
pub fn resolve(kind: &str) -> Option<Mapping> {
    if let Some(m) = mapping_for(kind) {
        return Some(m.clone());
    }
    let (key, variant) = normalize_expr_kind(kind)?;
    let mut m = mapping_for(key)?.clone();
    if let Some(vkey) = variant {
        if let Some(vstr) = m.variants.get(vkey) {
            let (mc, clause) = split_variant(vstr);
            m.metaclass = mc;
            if !clause.is_empty() {
                m.clause = clause;
            }
            // Variants are leaf expression kinds — operator injects / metadata
            // markers from the base row do not apply.
            m.injects = None;
            m.marker = None;
        }
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_normalizes_ast_expression_kinds() {
        // Operator/invocation/feature-ref AST kinds map to the IR rows.
        assert_eq!(resolve("binary").unwrap().metaclass, "OperatorExpression");
        assert_eq!(resolve("unary").unwrap().metaclass, "OperatorExpression");
        assert_eq!(resolve("call").unwrap().metaclass, "InvocationExpression");
        assert_eq!(
            resolve("identifier").unwrap().metaclass,
            "FeatureReferenceExpression"
        );
    }

    #[test]
    fn resolve_applies_literal_variants() {
        // int → base LiteralInteger; typed literals → their variant metaclass.
        assert_eq!(resolve("int_literal").unwrap().metaclass, "LiteralInteger");
        assert_eq!(resolve("real_literal").unwrap().metaclass, "LiteralRational");
        assert_eq!(resolve("string_literal").unwrap().metaclass, "LiteralString");
        assert_eq!(
            resolve("boolean_literal").unwrap().metaclass,
            "LiteralBoolean"
        );
    }

    #[test]
    fn resolve_passes_through_structural_kinds() {
        // Non-expression kinds resolve exactly as the table row.
        assert_eq!(resolve("part_def").unwrap().metaclass, "PartDefinition");
        assert!(resolve("definitely_not_a_kind").is_none());
    }

    #[test]
    fn table_parses_and_is_nonempty() {
        // Forces the embedded JSON through serde — a malformed table fails here.
        assert!(table().len() >= 30, "expected the full mapping table");
    }

    #[test]
    fn structural_anchor_rows_present_and_correct() {
        let p = mapping_for("part_def").expect("part_def mapped");
        assert_eq!(p.metaclass, "PartDefinition");
        assert_eq!(p.clause, "SysML 8.3.11.2");
        let c = mapping_for("calc_def").expect("calc_def mapped");
        assert_eq!(c.metaclass, "Function");
        assert_eq!(c.clause, "KerML 8.3.4.7.4");
    }

    #[test]
    fn p1_added_kinds_are_mapped() {
        assert_eq!(
            mapping_for("use_case_def").unwrap().metaclass,
            "UseCaseDefinition"
        );
        assert_eq!(
            mapping_for("allocation_def").unwrap().metaclass,
            "AllocationDefinition"
        );
        let actor = mapping_for("actor_def").unwrap();
        assert_eq!(actor.metaclass, "PartDefinition");
        assert!(actor.marker.as_deref().unwrap_or("").contains("actor"));
    }

    #[test]
    fn behavioral_and_expression_rows_present() {
        let d = mapping_for("decide_block").unwrap();
        assert_eq!(d.metaclass, "DecisionNode");
        assert!(d.injects.as_deref().unwrap_or("").contains("MergeNode"));
        assert_eq!(
            mapping_for("operator_expr").unwrap().metaclass,
            "OperatorExpression"
        );
    }

    #[test]
    fn every_ast_def_kind_is_covered() {
        // The 16 element-definition kinds (deal/src/ast.zig) must all map, so a
        // newly added element keyword can't silently ship without a SysML row.
        for kind in [
            "part_def",
            "port_def",
            "action_def",
            "state_def",
            "attribute_def",
            "item_def",
            "interface_def",
            "connection_def",
            "flow_def",
            "allocation_def",
            "requirement_def",
            "constraint_def",
            "need_def",
            "use_case_def",
            "actor_def",
            "calc_def",
        ] {
            assert!(mapping_for(kind).is_some(), "no SysML mapping for {kind}");
        }
    }

    #[test]
    fn unmapped_kind_returns_none() {
        assert!(mapping_for("definitely_not_a_kind").is_none());
    }
}
