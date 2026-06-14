//! Shared model-IR value resolution for the Phase 5 verify + simulate engines
//! (gap-closure 05-08).
//!
//! Both `deal check --verify` (verify.rs) and `deal simulate` (simulate.rs) need
//! to resolve a *model path* or a *requirement-attribute ref* to a concrete
//! numeric value:
//!
//!   - verify.rs resolves `evidence design { maps { <model_path> -> <field> } }`
//!     sources and criteria refs like `REQ_BAT_001.minCapacity`.
//!   - simulate.rs resolves `[[inputs]] model_path = "EnergyStorage.battery.packResistance"`.
//!
//! The DEAL **IR** (`deal_ir_json`) carries element keys but NOT their literal
//! values — the IR `payload` for an `attribute_usage` has `name` + `type_ref`
//! only, no value. The literal lives in the **AST** (`deal_ast_json`):
//!
//!   - requirement / part-def attributes: `attribute_usage.default_value`
//!   - model instance overrides: `component_instance.attrs[].fields[].value`
//!
//! So we build the value index from the AST, indexed by every useful path form,
//! and resolve a query by exact match then by *unique suffix* match.
//!
//! ## Unit-call canonicalization (05-08 decision D-08-A)
//!
//! A value literal is one of:
//!   - a bare numeric literal (`int_literal` / `float_literal`) → its f64 text
//!   - a unit call like `kWh(85)`, `ohm(0.05)`, `LPM(12)` → the f64 of the
//!     single numeric argument, **with the unit symbol discarded**.
//!
//! We deliberately do NOT convert to SI base units. Both operands of a showcase
//! comparison are authored in the *same* unit (e.g. `actualCapacity` is
//! `usableCapacity = kWh(85)` and the threshold `minCapacity = kWh(85)`), so
//! comparing the raw magnitudes (85 vs 85) is correct and `85 kWh >= 85 kWh`
//! evaluates PASS. The convention is symmetric: it is applied identically on
//! both sides of every comparison because both sides flow through this one
//! resolver.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// An index of resolved model values, keyed by multiple path forms.
///
/// Lookup precedence (see [`ModelValueIndex::resolve`]):
///   1. exact key match
///   2. unique suffix match (the query is a `.`-suffix of exactly one key)
#[derive(Debug, Default, Clone)]
pub struct ModelValueIndex {
    /// path-form → value. Multiple keys may map to the same logical attribute
    /// (e.g. `EnergyStorage.battery.usableCapacity`, `battery.usableCapacity`,
    /// `BatteryPack.usableCapacity`).
    values: BTreeMap<String, f64>,
}

impl ModelValueIndex {
    /// Build the index by parsing every `.deal`/`.dealx` file under `project_root`.
    ///
    /// Uses the `deal_parse` → `deal_ast_json` C ABI (clone-before-free, T-02-29).
    /// Files that fail to parse are skipped (best-effort, never panics).
    pub fn build(project_root: &Path) -> Self {
        let mut idx = ModelValueIndex::default();
        let mut files = Vec::new();
        collect_deal_files(project_root, &mut files);
        files.sort();
        for path in &files {
            if let Some(ast) = parse_ast(path) {
                idx.ingest_ast(&ast);
            }
        }
        idx
    }

    /// Resolve a query path (model path or `REQ.attr` ref) to a numeric value.
    ///
    /// Tries exact match first, then a unique `.`-suffix match. Returns `None`
    /// if the path is unknown or the suffix is ambiguous (matches >1 key).
    pub fn resolve(&self, query: &str) -> Option<f64> {
        // 1. Exact match.
        if let Some(v) = self.values.get(query) {
            return Some(*v);
        }

        // 2. Query is a dot-suffix of exactly one indexed key
        //    (query shorter/equal: e.g. `usableCapacity` → `BatteryPack.usableCapacity`).
        if let Some(v) = self.unique_key_with_suffix(query) {
            return Some(v);
        }

        // 3. A key is a dot-suffix of the query (query longer than the key:
        //    e.g. `EnergyStorage.battery.packResistance` → key `BatteryPack.packResistance`
        //    or bare `packResistance`). Try progressively shorter tails of the
        //    query, longest first, so the most specific indexed key wins.
        let segs: Vec<&str> = query.split('.').collect();
        for start in 1..segs.len() {
            let tail = segs[start..].join(".");
            if let Some(v) = self.values.get(&tail) {
                return Some(*v);
            }
            if let Some(v) = self.unique_key_with_suffix(&tail) {
                return Some(v);
            }
        }
        None
    }

    /// Return the value if exactly one indexed key ends with `.<suffix>` (or
    /// equals `suffix`), treating equal values as non-ambiguous. `None` if no
    /// match or genuinely ambiguous (>1 distinct value).
    fn unique_key_with_suffix(&self, suffix: &str) -> Option<f64> {
        let needle = format!(".{}", suffix);
        let mut hit: Option<f64> = None;
        for (k, v) in &self.values {
            if k == suffix || k.ends_with(&needle) {
                if hit.is_some() && hit != Some(*v) {
                    return None; // ambiguous
                }
                hit = Some(*v);
            }
        }
        hit
    }

    /// Number of indexed path forms (test/diagnostic helper).
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Insert a value under a path form (skips empty keys).
    fn insert(&mut self, key: String, value: f64) {
        if !key.is_empty() {
            self.values.insert(key, value);
        }
    }

    /// Walk the AST envelope (`{v, mode, filename, root}`) and ingest all
    /// attribute defaults + instance overrides.
    fn ingest_ast(&mut self, ast: &Value) {
        let root = ast.get("root").unwrap_or(ast);
        self.walk(root, &[]);
    }

    /// Recursive walker. `scope` is the chain of enclosing named scopes
    /// (requirement_def / part_def / system_block / subsystem_block /
    /// component_instance names).
    fn walk(&mut self, node: &Value, scope: &[String]) {
        match node {
            Value::Object(map) => {
                let kind = map.get("k").and_then(|v| v.as_str()).unwrap_or("");

                // Named scope nodes extend the scope chain for their children.
                let scope_name = match kind {
                    "requirement_def" | "part_def" | "system_block" | "subsystem_block"
                    | "component_instance" => map.get("name").and_then(|v| v.as_str()),
                    _ => None,
                };

                // component_instance carries inline attribute overrides.
                if kind == "component_instance" {
                    let alias = map.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let type_ref = map.get("type_ref").and_then(|v| v.as_str()).unwrap_or("");
                    self.ingest_instance_attrs(map.get("attrs"), scope, alias, type_ref);
                }

                // attribute_usage with a numeric default_value.
                if kind == "attribute_usage" {
                    if let Some(name) = map.get("name").and_then(|v| v.as_str()) {
                        if let Some(val) = map
                            .get("default_value")
                            .and_then(value_to_f64)
                        {
                            self.index_attr(scope, name, val);
                        }
                    }
                }

                // Descend, extending the scope if this is a named scope node.
                if let Some(name) = scope_name {
                    let mut child_scope = scope.to_vec();
                    child_scope.push(name.to_string());
                    for v in map.values() {
                        self.walk(v, &child_scope);
                    }
                } else {
                    for v in map.values() {
                        self.walk(v, scope);
                    }
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    self.walk(v, scope);
                }
            }
            _ => {}
        }
    }

    /// Index an `attribute_usage` default under several path forms:
    ///   - `<innermost-scope>.<attr>`  (e.g. `REQ_BAT_001.minCapacity`,
    ///     `BatteryPack.usableCapacity`)
    ///   - `<attr>` bare (suffix-resolvable)
    fn index_attr(&mut self, scope: &[String], attr: &str, val: f64) {
        if let Some(owner) = scope.last() {
            self.insert(format!("{}.{}", owner, attr), val);
        }
        // bare attr is also suffix-resolvable but only insert if not shadowing.
        self.values.entry(attr.to_string()).or_insert(val);
    }

    /// Ingest a `component_instance`'s `attrs` (list of object_literal with
    /// `fields[] = {key, value}`). The instance lives at `scope` (the subsystem
    /// chain) under `alias`; we index by the full instance path
    /// `Subsystem.alias.key` plus shorter forms.
    fn ingest_instance_attrs(
        &mut self,
        attrs: Option<&Value>,
        scope: &[String],
        alias: &str,
        type_ref: &str,
    ) {
        let attrs = match attrs.and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return,
        };
        // The registry model paths drop the top-level system name
        // (`EVPlatform`), so build the subsystem-rooted chain. scope is e.g.
        // ["EVPlatform", "EnergyStorage"]; we want "EnergyStorage".
        let subsystem = scope
            .iter()
            .rev()
            .find(|s| {
                // skip the top system name; use the nearest subsystem.
                !s.is_empty()
            })
            .cloned()
            .unwrap_or_default();

        for obj in attrs {
            let fields = match obj.get("fields").and_then(|v| v.as_array()) {
                Some(f) => f,
                None => continue,
            };
            for field in fields {
                let key = match field.get("key").and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => continue,
                };
                if let Some(val) = field.get("value").and_then(value_to_f64) {
                    // Full instance path: EnergyStorage.battery.usableCapacity
                    if !subsystem.is_empty() {
                        self.insert(format!("{}.{}.{}", subsystem, alias, key), val);
                    }
                    // alias-rooted: battery.usableCapacity
                    self.insert(format!("{}.{}", alias, key), val);
                    // part-def-rooted: BatteryPack.usableCapacity
                    if !type_ref.is_empty() {
                        self.insert(format!("{}.{}", type_ref, key), val);
                    }
                    // bare attr (only if not already a stronger binding)
                    self.values.entry(key.to_string()).or_insert(val);
                }
            }
        }
    }
}

/// Convert an AST value node to f64.
///
/// Handles:
///   - `int_literal` / `float_literal` → parse `text`
///   - `call` with a single numeric arg → the arg's f64 (unit symbol discarded;
///     see module docs / D-08-A)
fn value_to_f64(node: &Value) -> Option<f64> {
    let k = node.get("k").and_then(|v| v.as_str())?;
    match k {
        "int_literal" | "float_literal" | "real_literal" | "number" | "numeric_literal" => {
            node.get("text").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
                .or_else(|| node.get("value").and_then(|v| v.as_f64()))
        }
        "call" => {
            // Unit call: callee is the unit symbol, single numeric arg.
            let args = node.get("args").and_then(|v| v.as_array())?;
            if args.len() != 1 {
                return None;
            }
            value_to_f64(&args[0])
        }
        "unary" => {
            // Signed literal, e.g. `degC(-30)` → call arg is unary neg over 30.
            let v = value_to_f64(node.get("operand")?)?;
            match node.get("op").and_then(|o| o.as_str()) {
                Some("neg") => Some(-v),
                _ => Some(v),
            }
        }
        // Some encodings inline the literal as a bare JSON number under `value`.
        _ => node.get("value").and_then(|v| v.as_f64()),
    }
}

/// Parse a file via the C ABI and return its AST JSON value (clone-before-free).
fn parse_ast(path: &Path) -> Option<Value> {
    use deal_ffi as ffi;
    let bytes = std::fs::read(path).ok()?;
    let fname = path.to_string_lossy();
    let handle = unsafe {
        ffi::deal_parse(bytes.as_ptr(), bytes.len(), fname.as_bytes().as_ptr(), fname.len())
    };
    if handle.is_null() {
        return None;
    }
    let mut out_ptr: *const u8 = std::ptr::null();
    let mut out_len: usize = 0;
    let ast = unsafe {
        let ok = ffi::deal_ast_json(handle, &mut out_ptr, &mut out_len);
        if !ok || out_ptr.is_null() || out_len == 0 {
            ffi::deal_free(handle);
            return None;
        }
        let cloned = std::slice::from_raw_parts(out_ptr, out_len).to_vec();
        ffi::deal_free(handle);
        cloned
    };
    serde_json::from_slice(&ast).ok()
}

/// Collect `.deal`/`.dealx` files under `root` (skips `.deal/`, `target/`,
/// dot-dirs). Shared shape with simulate.rs::collect_deal_files.
pub fn collect_deal_files(root: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == ".deal" || name == "target" || name.starts_with('.') {
                continue;
            }
            collect_deal_files(&path, files);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "deal" || ext == "dealx" {
                files.push(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn showcase_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("spec/examples/showcase")
    }

    #[test]
    fn resolves_requirement_attribute() {
        let idx = ModelValueIndex::build(&showcase_root());
        // REQ_BAT_001.minCapacity = kWh(85)
        let v = idx.resolve("REQ_BAT_001.minCapacity").expect("minCapacity");
        assert_eq!(v, 85.0, "minCapacity must be 85 (kWh magnitude)");
    }

    #[test]
    fn resolves_instance_override() {
        let idx = ModelValueIndex::build(&showcase_root());
        // EnergyStorage.battery.usableCapacity = kWh(85)
        let v = idx
            .resolve("EnergyStorage.battery.usableCapacity")
            .expect("usableCapacity");
        assert_eq!(v, 85.0, "usableCapacity must be 85 (kWh magnitude)");
    }

    #[test]
    fn resolves_part_def_attribute_via_long_query() {
        // Sim registry uses the instance-hierarchy form
        // `EnergyStorage.battery.packResistance`, which is LONGER than the
        // indexed part-def key `BatteryPack.packResistance`. The progressive
        // tail match must still find it (05-08 simulate input resolution).
        let idx = ModelValueIndex::build(&showcase_root());
        assert_eq!(
            idx.resolve("EnergyStorage.battery.packResistance"),
            Some(0.05),
            "packResistance must resolve via tail match"
        );
        assert_eq!(
            idx.resolve("EnergyStorage.battery.totalCurrent"),
            Some(250.0),
            "totalCurrent must resolve"
        );
        assert_eq!(
            idx.resolve("Thermal.coolantSupply.flowRate"),
            Some(30.0),
            "coolant flowRate must resolve"
        );
    }

    #[test]
    fn pass_capacity_compares_equal() {
        // The whole point: 85 kWh >= 85 kWh must hold with identical magnitudes.
        let idx = ModelValueIndex::build(&showcase_root());
        let actual = idx.resolve("EnergyStorage.battery.usableCapacity").unwrap();
        let threshold = idx.resolve("REQ_BAT_001.minCapacity").unwrap();
        assert!(actual >= threshold, "REQ_BAT_001 criterion must be satisfiable");
    }
}
