//! Defensive test for the alphabetical-key invariant (D-18).
//!
//! This test guards Pitfall 1 from RESEARCH.md:
//! "A transitive dep enables the `preserve_order` feature, swapping BTreeMap
//! for IndexMap. JSON output then preserves *insertion* order, breaking D-18's
//! alphabetical-key invariant for the SysML emitter and --json envelope."
//!
//! If this test ever fails, a transitive dep has flipped `serde_json/preserve_order`.
//! Locate the offender with:
//!   cargo tree -e features --workspace | grep preserve_order

#[test]
fn serde_json_emits_alphabetical_keys() {
    // Serialize with keys in reverse-alphabetical insertion order.
    // serde_json's default Map backend is BTreeMap → alphabetical output.
    // If preserve_order is active (IndexMap backend), output would be {"z":1,"a":2}.
    let value = serde_json::json!({"z": 1, "a": 2});
    let serialized = serde_json::to_string(&value).expect("serde_json serialization failed");

    assert_eq!(
        serialized, r#"{"a":2,"z":1}"#,
        "serde_json key order invariant broken: expected alphabetical ({{\"a\":2,\"z\":1}}) \
         but got {serialized:?}. A transitive dep may have enabled serde_json/preserve_order. \
         Run `cargo tree -e features --workspace | grep preserve_order` to locate the offender. \
         (D-18 / Pitfall 1 / RESEARCH.md)",
    );
}
