//! ReqIF 1.2 XML emitter.
//!
//! Consumes a DEAL IR v0 JSON document (produced by `deal_ir_json` C ABI)
//! and emits a ReqIF 1.2 XML document per D-59 + D-61.
//!
//! Key design decisions:
//!   D-19: Rust CLI crate owns this emitter; walks IR via serde_json::Value.
//!   D-59: Only requirement_def/need_def nodes are exported (structure not exported).
//!         @trace:<<satisfies>>/<<derives from>>/[<satisfy>]/[<allocate>] edges → SpecRelations.
//!   D-61: Output is .reqifz zip archive containing one model.reqif XML entry.
//!   D-18: Collections sorted by ID before writing (byte-deterministic output).
//!   T-4-13: XML metacharacters in requirement text are escaped by quick-xml Writer
//!           (never custom string-format XML).
//!   FIXED-TIMESTAMP: CREATION-TIME is a pinned constant (D-18 golden-diff stability).

use std::io::{Cursor, Write as IoWrite};
use std::path::Path;

use anyhow::{anyhow, Context as _};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use serde_json::Value;
use uuid::Uuid;

use crate::reqif_schema;

// ─── UUID namespace ───────────────────────────────────────────────────────────

/// ReqIF UUID namespace for DEAL header/spec-type identifier synthesis.
///
/// Distinct from SYSML_NAMESPACE (sysml_v2.rs). MUST NOT CHANGE across releases.
const REQIF_UUID_NAMESPACE: Uuid = Uuid::from_u128(0x3a8f_1c2e_5b47_4d91_8e73_9f12_3c56_7ab8);

/// Fixed CREATION-TIME constant for byte-deterministic output (D-18).
///
/// Using `now()` would break golden-fixture diffs. This is the canonical
/// "DEAL build epoch" timestamp — not a real creation time.
const FIXED_CREATION_TIME: &str = "2026-01-01T00:00:00.000+00:00";

/// OMG ReqIF 1.2 XML namespace.
const REQIF_NAMESPACE: &str = "http://www.omg.org/spec/ReqIF/20110401/reqif.xsd";

// ─── ID derivation ────────────────────────────────────────────────────────────

/// Convert a DEAL dot-path ID to a ReqIF XML identifier.
///
/// ReqIF identifiers must be valid XML IDs (no dots).
/// Strategy: replace '.' with '_' and prefix with 'DEAL_'.
///
/// Example: `requirements.system.REQ_BAT_001` → `DEAL_requirements_system_REQ_BAT_001`
pub fn deal_id_to_reqif_id(qualified_path: &str) -> String {
    format!("DEAL_{}", qualified_path.replace('.', "_"))
}

/// Synthesize a UUID v5 from a label using the REQIF namespace.
///
/// Used for fixed identifiers (header, spec types, attribute definitions)
/// that are stable across runs.
fn reqif_uuid(label: &str) -> String {
    Uuid::new_v5(&REQIF_UUID_NAMESPACE, label.as_bytes()).to_string()
}

// ─── IR types ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct IrNode<'a> {
    id: &'a str,
    kind: &'a str,
    payload: &'a Value,
}

#[derive(Debug)]
struct IrEdge<'a> {
    src: &'a str,
    dst: &'a str,
    kind: &'a str,
}

// ─── ReqIF element structs ────────────────────────────────────────────────────

struct SpecObject {
    identifier: String,
    long_name: String,
    req_text: String,
    threshold: Option<String>,
    operator: Option<String>,
    method: Option<String>,
}

struct SpecRelation {
    identifier: String,
    source_id: String,
    target_id: String,
    /// WR-01: the original trace edge kind (satisfies / traces / derives_from /
    /// allocated_to) so the relation can reference its own SPEC-RELATION-TYPE
    /// instead of collapsing every kind into "Satisfies".
    kind: String,
}

/// WR-01: map a trace edge kind to a stable SPEC-RELATION-TYPE label + LONG-NAME.
///
/// Each distinct kind gets its own SPEC-RELATION-TYPE so a `derives_from` or
/// `allocated_to` edge is not silently exported as a "Satisfies" relation
/// (semantic data loss). The label feeds `reqif_uuid` for a stable IDENTIFIER.
fn relation_type_info(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "satisfies" => ("SPEC-RELATION-TYPE-Satisfies", "SatisfiesRelation"),
        "traces" => ("SPEC-RELATION-TYPE-Traces", "TracesRelation"),
        "derives_from" => ("SPEC-RELATION-TYPE-DerivesFrom", "DerivesFromRelation"),
        "allocated_to" => ("SPEC-RELATION-TYPE-AllocatedTo", "AllocatedToRelation"),
        // Fallback: unknown kinds map to a generic trace type rather than
        // masquerading as Satisfies.
        _ => ("SPEC-RELATION-TYPE-Trace", "TraceRelation"),
    }
}

// ─── Top-level emitter ────────────────────────────────────────────────────────

/// Emit a ReqIF 1.2 XML document from an IR v0 JSON document.
///
/// Returns the raw XML bytes. The XML is structurally valid per D-58/OQ-1.
///
/// D-59: only `requirement_def` and `need_def` nodes are emitted as SpecObjects.
///       `part_def`, `port_def`, and all other structural nodes are skipped.
///
/// D-18: SpecObjects and SpecRelations are sorted by identifier before writing.
pub fn emit(ir_json: &Value) -> anyhow::Result<Vec<u8>> {
    // Validate IR envelope shape.
    let elements_map = ir_json
        .get("elements")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("IR JSON missing 'elements' object"))?;

    let edges_arr = ir_json
        .get("edges")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("IR JSON missing 'edges' array"))?;

    // Parse edges.
    let mut edges: Vec<IrEdge> = Vec::with_capacity(edges_arr.len());
    for edge_val in edges_arr {
        let src = edge_val["src"].as_str().unwrap_or("");
        let dst = edge_val["dst"].as_str().unwrap_or("");
        let kind = edge_val["kind"].as_str().unwrap_or("");
        if !src.is_empty() && !dst.is_empty() && !kind.is_empty() {
            edges.push(IrEdge { src, dst, kind });
        }
    }

    // Parse nodes — filter to requirement_def / need_def only (D-59).
    let mut nodes: Vec<IrNode> = Vec::with_capacity(elements_map.len());
    for (id, node_val) in elements_map {
        let kind = node_val["kind"].as_str().unwrap_or("");
        let payload = &node_val["payload"];
        nodes.push(IrNode {
            id: id.as_str(),
            kind,
            payload,
        });
    }

    // Sort all nodes by id (D-18 deterministic order).
    nodes.sort_by_key(|n| n.id);

    // Collect requirement/need nodes (D-59 filter).
    let req_nodes: Vec<&IrNode> = nodes
        .iter()
        .filter(|n| n.kind == "requirement_def" || n.kind == "need_def")
        .collect();

    // Build SpecObjects from requirement/need nodes.
    let mut spec_objects: Vec<SpecObject> =
        req_nodes.iter().map(|n| node_to_spec_object(n)).collect();

    // Sort by identifier (D-18).
    spec_objects.sort_by(|a, b| a.identifier.cmp(&b.identifier));

    // Build SpecRelations from trace edges.
    // Edges of kind "satisfies", "traces", "derives_from", "allocated_to"
    // whose source OR target is a requirement/need node.
    //
    // Note: the IR lowering may emit unqualified target IDs for annotation targets
    // (e.g., `@trace:<<satisfies>> REQ_CAPACITY` produces dst="REQ_CAPACITY" not
    // "gold.reqif02.trace.REQ_CAPACITY"). We check both the full ID and the last
    // path component (local name) when matching against req_ids.
    let req_ids: std::collections::HashSet<&str> = req_nodes.iter().map(|n| n.id).collect();

    // Also build a set of local (unqualified) names for requirement nodes.
    let req_local_names: std::collections::HashSet<&str> = req_nodes
        .iter()
        .map(|n| n.id.rsplit('.').next().unwrap_or(n.id))
        .collect();

    // Check if an edge endpoint matches a requirement node (qualified or unqualified).
    let matches_req = |id: &str| -> bool { req_ids.contains(id) || req_local_names.contains(id) };

    let trace_kinds = ["satisfies", "traces", "derives_from", "allocated_to"];
    let mut spec_relations: Vec<SpecRelation> = edges
        .iter()
        .filter(|e| trace_kinds.contains(&e.kind))
        // CR-02: only emit a SpecRelation when BOTH endpoints are emitted
        // SpecObjects (requirement/need nodes). The D-59 filter drops
        // non-requirement nodes (e.g. a `part_def`) from SPEC-OBJECTS, so a
        // relation that referenced such an endpoint would contain a
        // SPEC-OBJECT-REF pointing at a SpecObject that does not exist in the
        // document — a dangling reference that real ReqIF tools (DOORS,
        // Polarion) reject or silently drop. Requiring both endpoints to be
        // requirements guarantees referential integrity of every emitted ref.
        .filter(|e| matches_req(e.src) && matches_req(e.dst))
        .map(|e| SpecRelation {
            identifier: format!(
                "DEAL_REL_{}_{}",
                e.src.replace('.', "_"),
                e.dst.replace('.', "_")
            ),
            source_id: deal_id_to_reqif_id(e.src),
            target_id: deal_id_to_reqif_id(e.dst),
            kind: e.kind.to_string(),
        })
        .collect();

    // Sort by identifier (D-18).
    spec_relations.sort_by(|a, b| a.identifier.cmp(&b.identifier));

    // WR-01: collect the distinct relation kinds actually present, sorted for
    // deterministic SPEC-RELATION-TYPE emission order (D-18). Each kind gets its
    // own SPEC-RELATION-TYPE so the original edge semantics are preserved.
    let mut relation_kinds: Vec<String> = spec_relations.iter().map(|r| r.kind.clone()).collect();
    relation_kinds.sort();
    relation_kinds.dedup();

    // Map each kind → its stable SPEC-RELATION-TYPE IDENTIFIER (uuid v5 of the label).
    let relation_type_ids: std::collections::BTreeMap<String, String> = relation_kinds
        .iter()
        .map(|k| {
            let (label, _long_name) = relation_type_info(k);
            (k.clone(), reqif_uuid(label))
        })
        .collect();

    // Fixed UUIDs for spec types and attribute definitions (stable across runs).
    let sot_req_id = reqif_uuid("SPEC-OBJECT-TYPE-Requirement");
    let ads_text_id = reqif_uuid("ATTRIBUTE-DEFINITION-STRING-ReqText");
    let ads_threshold_id = reqif_uuid("ATTRIBUTE-DEFINITION-STRING-Threshold");
    let ads_operator_id = reqif_uuid("ATTRIBUTE-DEFINITION-STRING-Operator");
    let ads_method_id = reqif_uuid("ATTRIBUTE-DEFINITION-STRING-Method");
    let spec_id = reqif_uuid("SPECIFICATION-001");
    let hdr_id = reqif_uuid("REQ-IF-HEADER-001");

    // Write XML via quick-xml Writer (T-4-13: auto-escapes metacharacters).
    let mut xml_buf: Vec<u8> = Vec::new();
    let mut writer = Writer::new_with_indent(Cursor::new(&mut xml_buf), b' ', 2);

    // XML declaration.
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .context("write XML declaration")?;
    write_newline(&mut writer)?;

    // <REQ-IF xmlns="...">
    let mut reqif_start = BytesStart::new("REQ-IF");
    reqif_start.push_attribute(("xmlns", REQIF_NAMESPACE));
    writer
        .write_event(Event::Start(reqif_start))
        .context("write REQ-IF start")?;

    // <THE-HEADER>
    writer
        .write_event(Event::Start(BytesStart::new("THE-HEADER")))
        .context("write THE-HEADER")?;

    // <REQ-IF-HEADER IDENTIFIER="..." CREATION-TIME="..." REQ-IF-TOOL-ID="deal" />
    let mut hdr = BytesStart::new("REQ-IF-HEADER");
    hdr.push_attribute(("IDENTIFIER", hdr_id.as_str()));
    hdr.push_attribute(("CREATION-TIME", FIXED_CREATION_TIME));
    hdr.push_attribute(("REQ-IF-TOOL-ID", "deal"));
    hdr.push_attribute(("REQ-IF-VERSION", "1.2"));
    writer
        .write_event(Event::Empty(hdr))
        .context("write REQ-IF-HEADER")?;

    writer
        .write_event(Event::End(BytesEnd::new("THE-HEADER")))
        .context("write /THE-HEADER")?;

    // <CORE-CONTENT>
    writer
        .write_event(Event::Start(BytesStart::new("CORE-CONTENT")))
        .context("write CORE-CONTENT")?;

    // <SPEC-TYPES>
    writer
        .write_event(Event::Start(BytesStart::new("SPEC-TYPES")))
        .context("write SPEC-TYPES")?;

    // SPEC-OBJECT-TYPE for requirements.
    {
        let mut sot = BytesStart::new("SPEC-OBJECT-TYPE");
        sot.push_attribute(("IDENTIFIER", sot_req_id.as_str()));
        sot.push_attribute(("LONG-NAME", "RequirementType"));
        writer
            .write_event(Event::Start(sot))
            .context("write SPEC-OBJECT-TYPE")?;

        writer
            .write_event(Event::Start(BytesStart::new("SPEC-ATTRIBUTES")))
            .context("write SPEC-ATTRIBUTES")?;

        // ReqText attribute definition.
        let mut ads_text = BytesStart::new("ATTRIBUTE-DEFINITION-STRING");
        ads_text.push_attribute(("IDENTIFIER", ads_text_id.as_str()));
        ads_text.push_attribute(("LONG-NAME", "ReqText"));
        writer
            .write_event(Event::Empty(ads_text))
            .context("write ReqText attr def")?;

        // Threshold attribute definition.
        let mut ads_thresh = BytesStart::new("ATTRIBUTE-DEFINITION-STRING");
        ads_thresh.push_attribute(("IDENTIFIER", ads_threshold_id.as_str()));
        ads_thresh.push_attribute(("LONG-NAME", "Threshold"));
        writer
            .write_event(Event::Empty(ads_thresh))
            .context("write Threshold attr def")?;

        // Operator attribute definition.
        let mut ads_op = BytesStart::new("ATTRIBUTE-DEFINITION-STRING");
        ads_op.push_attribute(("IDENTIFIER", ads_operator_id.as_str()));
        ads_op.push_attribute(("LONG-NAME", "Operator"));
        writer
            .write_event(Event::Empty(ads_op))
            .context("write Operator attr def")?;

        // Method attribute definition.
        let mut ads_meth = BytesStart::new("ATTRIBUTE-DEFINITION-STRING");
        ads_meth.push_attribute(("IDENTIFIER", ads_method_id.as_str()));
        ads_meth.push_attribute(("LONG-NAME", "VerificationMethod"));
        writer
            .write_event(Event::Empty(ads_meth))
            .context("write Method attr def")?;

        writer
            .write_event(Event::End(BytesEnd::new("SPEC-ATTRIBUTES")))
            .context("write /SPEC-ATTRIBUTES")?;

        writer
            .write_event(Event::End(BytesEnd::new("SPEC-OBJECT-TYPE")))
            .context("write /SPEC-OBJECT-TYPE")?;
    }

    // WR-01: one SPEC-RELATION-TYPE per distinct trace kind present (sorted),
    // so derives_from / allocated_to / traces are not collapsed into Satisfies.
    for kind in &relation_kinds {
        let (_label, long_name) = relation_type_info(kind);
        let id = &relation_type_ids[kind];
        let mut srt = BytesStart::new("SPEC-RELATION-TYPE");
        srt.push_attribute(("IDENTIFIER", id.as_str()));
        srt.push_attribute(("LONG-NAME", long_name));
        writer
            .write_event(Event::Empty(srt))
            .context("write SPEC-RELATION-TYPE")?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("SPEC-TYPES")))
        .context("write /SPEC-TYPES")?;

    // <SPEC-OBJECTS>
    writer
        .write_event(Event::Start(BytesStart::new("SPEC-OBJECTS")))
        .context("write SPEC-OBJECTS")?;

    for so in &spec_objects {
        let mut so_elem = BytesStart::new("SPEC-OBJECT");
        so_elem.push_attribute(("IDENTIFIER", so.identifier.as_str()));
        so_elem.push_attribute(("LONG-NAME", so.long_name.as_str()));
        writer
            .write_event(Event::Start(so_elem))
            .context("write SPEC-OBJECT")?;

        // <TYPE>
        writer
            .write_event(Event::Start(BytesStart::new("TYPE")))
            .context("write TYPE")?;
        writer
            .write_event(Event::Start(BytesStart::new("SPEC-OBJECT-TYPE-REF")))
            .context("write SPEC-OBJECT-TYPE-REF start")?;
        writer
            .write_event(Event::Text(BytesText::new(sot_req_id.as_str())))
            .context("write SPEC-OBJECT-TYPE-REF text")?;
        writer
            .write_event(Event::End(BytesEnd::new("SPEC-OBJECT-TYPE-REF")))
            .context("write /SPEC-OBJECT-TYPE-REF")?;
        writer
            .write_event(Event::End(BytesEnd::new("TYPE")))
            .context("write /TYPE")?;

        // <VALUES>
        writer
            .write_event(Event::Start(BytesStart::new("VALUES")))
            .context("write VALUES")?;

        // ReqText value.
        emit_attribute_value_string(&mut writer, &ads_text_id, &so.req_text)?;

        // Verification fields (only if present).
        if let Some(ref v) = so.threshold {
            emit_attribute_value_string(&mut writer, &ads_threshold_id, v)?;
        }
        if let Some(ref v) = so.operator {
            emit_attribute_value_string(&mut writer, &ads_operator_id, v)?;
        }
        if let Some(ref v) = so.method {
            emit_attribute_value_string(&mut writer, &ads_method_id, v)?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("VALUES")))
            .context("write /VALUES")?;

        writer
            .write_event(Event::End(BytesEnd::new("SPEC-OBJECT")))
            .context("write /SPEC-OBJECT")?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("SPEC-OBJECTS")))
        .context("write /SPEC-OBJECTS")?;

    // <SPEC-RELATIONS>
    writer
        .write_event(Event::Start(BytesStart::new("SPEC-RELATIONS")))
        .context("write SPEC-RELATIONS")?;

    for sr in &spec_relations {
        let mut sr_elem = BytesStart::new("SPEC-RELATION");
        sr_elem.push_attribute(("IDENTIFIER", sr.identifier.as_str()));
        writer
            .write_event(Event::Start(sr_elem))
            .context("write SPEC-RELATION")?;

        // <TYPE>
        writer
            .write_event(Event::Start(BytesStart::new("TYPE")))
            .context("write SPEC-RELATION TYPE")?;
        writer
            .write_event(Event::Start(BytesStart::new("SPEC-RELATION-TYPE-REF")))
            .context("write SPEC-RELATION-TYPE-REF start")?;
        // WR-01: reference the SPEC-RELATION-TYPE matching this edge's kind.
        let srt_ref_id = &relation_type_ids[&sr.kind];
        writer
            .write_event(Event::Text(BytesText::new(srt_ref_id.as_str())))
            .context("write SPEC-RELATION-TYPE-REF text")?;
        writer
            .write_event(Event::End(BytesEnd::new("SPEC-RELATION-TYPE-REF")))
            .context("write /SPEC-RELATION-TYPE-REF")?;
        writer
            .write_event(Event::End(BytesEnd::new("TYPE")))
            .context("write /SPEC-RELATION TYPE")?;

        // <SOURCE>
        writer
            .write_event(Event::Start(BytesStart::new("SOURCE")))
            .context("write SOURCE")?;
        writer
            .write_event(Event::Start(BytesStart::new("SPEC-OBJECT-REF")))
            .context("write SPEC-OBJECT-REF start (src)")?;
        writer
            .write_event(Event::Text(BytesText::new(sr.source_id.as_str())))
            .context("write SPEC-OBJECT-REF text (src)")?;
        writer
            .write_event(Event::End(BytesEnd::new("SPEC-OBJECT-REF")))
            .context("write /SPEC-OBJECT-REF (src)")?;
        writer
            .write_event(Event::End(BytesEnd::new("SOURCE")))
            .context("write /SOURCE")?;

        // <TARGET>
        writer
            .write_event(Event::Start(BytesStart::new("TARGET")))
            .context("write TARGET")?;
        writer
            .write_event(Event::Start(BytesStart::new("SPEC-OBJECT-REF")))
            .context("write SPEC-OBJECT-REF start (dst)")?;
        writer
            .write_event(Event::Text(BytesText::new(sr.target_id.as_str())))
            .context("write SPEC-OBJECT-REF text (dst)")?;
        writer
            .write_event(Event::End(BytesEnd::new("SPEC-OBJECT-REF")))
            .context("write /SPEC-OBJECT-REF (dst)")?;
        writer
            .write_event(Event::End(BytesEnd::new("TARGET")))
            .context("write /TARGET")?;

        writer
            .write_event(Event::End(BytesEnd::new("SPEC-RELATION")))
            .context("write /SPEC-RELATION")?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("SPEC-RELATIONS")))
        .context("write /SPEC-RELATIONS")?;

    // <SPECIFICATIONS>
    writer
        .write_event(Event::Start(BytesStart::new("SPECIFICATIONS")))
        .context("write SPECIFICATIONS")?;

    // One Specification collecting all SpecObjects in order.
    {
        let mut spec_elem = BytesStart::new("SPECIFICATION");
        spec_elem.push_attribute(("IDENTIFIER", spec_id.as_str()));
        spec_elem.push_attribute(("LONG-NAME", "DEAL Requirements"));
        writer
            .write_event(Event::Start(spec_elem))
            .context("write SPECIFICATION")?;

        if !spec_objects.is_empty() {
            writer
                .write_event(Event::Start(BytesStart::new("CHILDREN")))
                .context("write CHILDREN")?;

            for so in &spec_objects {
                writer
                    .write_event(Event::Start(BytesStart::new("SPEC-HIERARCHY")))
                    .context("write SPEC-HIERARCHY")?;

                writer
                    .write_event(Event::Start(BytesStart::new("OBJECT")))
                    .context("write OBJECT")?;
                writer
                    .write_event(Event::Start(BytesStart::new("SPEC-OBJECT-REF")))
                    .context("write SPEC-OBJECT-REF start (spec)")?;
                writer
                    .write_event(Event::Text(BytesText::new(so.identifier.as_str())))
                    .context("write SPEC-OBJECT-REF text (spec)")?;
                writer
                    .write_event(Event::End(BytesEnd::new("SPEC-OBJECT-REF")))
                    .context("write /SPEC-OBJECT-REF (spec)")?;
                writer
                    .write_event(Event::End(BytesEnd::new("OBJECT")))
                    .context("write /OBJECT")?;

                writer
                    .write_event(Event::End(BytesEnd::new("SPEC-HIERARCHY")))
                    .context("write /SPEC-HIERARCHY")?;
            }

            writer
                .write_event(Event::End(BytesEnd::new("CHILDREN")))
                .context("write /CHILDREN")?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("SPECIFICATION")))
            .context("write /SPECIFICATION")?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("SPECIFICATIONS")))
        .context("write /SPECIFICATIONS")?;

    writer
        .write_event(Event::End(BytesEnd::new("CORE-CONTENT")))
        .context("write /CORE-CONTENT")?;

    writer
        .write_event(Event::End(BytesEnd::new("REQ-IF")))
        .context("write /REQ-IF")?;

    // Flush the Writer (Cursor into xml_buf).
    drop(writer);

    // Append a trailing newline for file cleanliness.
    xml_buf.push(b'\n');

    Ok(xml_buf)
}

// ─── emit_from_bytes ──────────────────────────────────────────────────────────

/// Invoke the full IR → ReqIF 1.2 pipeline.
///
/// 1. Parses IR JSON from `ir_json_bytes`.
/// 2. Emits ReqIF 1.2 XML via `emit()`.
/// 3. Structurally validates the XML (hard gate, D-58/OQ-1).
/// 4. Packages the XML as a `.reqifz` zip archive (D-61).
/// 5. Writes the archive to `output_path`.
///
/// Returns `(req_count, rel_count)` — the number of SpecObjects and SpecRelations
/// written, for the CLI success message.
///
/// Pitfall 3 mitigation: `ir_json_bytes` must already be cloned before the caller
/// calls `deal_free`; this function takes `&[u8]` (already cloned).
pub fn emit_from_bytes(ir_json_bytes: &[u8], output_path: &Path) -> anyhow::Result<(usize, usize)> {
    let ir_value: Value = serde_json::from_slice(ir_json_bytes)
        .with_context(|| "failed to parse IR JSON from deal_ir_json output")?;

    // Emit ReqIF XML.
    let xml_bytes = emit(&ir_value)?;

    // T-4-12: SHA256 tamper-detection of the XSD bundle before the structural
    // gate. Fails closed — a missing or tampered bundle aborts the build.
    // DEAL_REQIF_SCHEMAS_DIR overrides bundle location for deployed binaries.
    let bundle_dir = reqif_schema::locate_reqif_schemas_dir()?;
    reqif_schema::load_reqif_schemas(&bundle_dir)?;

    // Hard gate: structural validation (OQ-1).
    reqif_schema::validate_reqif_xml(&xml_bytes).map_err(|violations| {
        anyhow!(
            "ReqIF structural validation failed ({} violation(s)):\n{}",
            violations.len(),
            violations.join("\n")
        )
    })?;

    // Count requirements and relations for the success message.
    let req_count = count_spec_objects(&xml_bytes);
    let rel_count = count_spec_relations(&xml_bytes);

    // Package as .reqifz zip (D-61).
    let reqifz_bytes = wrap_in_reqifz(&xml_bytes)?;

    // Write output file.
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output directory {}", parent.display()))?;
    }

    std::fs::write(output_path, &reqifz_bytes)
        .with_context(|| format!("cannot write reqifz to {}", output_path.display()))?;

    Ok((req_count, rel_count))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Convert an IR node to a SpecObject.
///
/// Extracts: doc comment → ReqText; verification{} fields → typed attributes.
///
/// Note: DEAL IR v0 is comment-free (D-25) — doc-comment → documentation lowering
/// is deferred to a future ADR. Until that lowering is implemented, the requirement
/// name is used as the ReqText value so the ReqIF artifact has a meaningful label.
fn node_to_spec_object(node: &IrNode) -> SpecObject {
    let identifier = deal_id_to_reqif_id(node.id);
    // Use the last path component as the display name.
    let long_name = node.id.rsplit('.').next().unwrap_or(node.id).to_string();

    // Extract doc comment text (JSDoc → ReqText) if present in payload.
    // If absent (D-25 IR is comment-free), fall back to the requirement name.
    let doc_text = extract_doc_text(node.payload);
    let req_text = if doc_text.is_empty() {
        // D-25: IR is comment-free; use name as placeholder until doc lowering is implemented.
        let name = node.payload["name"].as_str().unwrap_or(&long_name);
        name.to_string()
    } else {
        doc_text
    };

    // Extract verification block fields from payload.
    let threshold = extract_string_field(node.payload, "threshold");
    let operator = extract_string_field(node.payload, "operator");

    // "accepts" is an array of method strings; join them.
    let method = node
        .payload
        .get("verification")
        .and_then(|v| v.get("accepts"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty());

    SpecObject {
        identifier,
        long_name,
        req_text,
        threshold,
        operator,
        method,
    }
}

/// Extract the doc comment text from a payload node.
fn extract_doc_text(payload: &Value) -> String {
    // Try "doc_comment" field (string).
    if let Some(Value::String(s)) = payload.get("doc_comment") {
        return clean_doc_comment(s);
    }
    // Try "doc" object with "text" field.
    if let Some(doc) = payload.get("doc") {
        if let Some(Value::String(s)) = doc.get("text") {
            return clean_doc_comment(s);
        }
    }
    // Try direct "text" field (some payloads store it here).
    if let Some(Value::String(s)) = payload.get("text") {
        return clean_doc_comment(s);
    }
    String::new()
}

/// Strip JSDoc markers from a doc comment string.
fn clean_doc_comment(raw: &str) -> String {
    let s = raw.trim();
    // Remove leading /** and trailing */
    let s = s.strip_prefix("/**").unwrap_or(s);
    let s = s.strip_suffix("*/").unwrap_or(s);
    // Remove leading * on each line (JSDoc continuation style).
    s.lines()
        .map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("* ")
                .unwrap_or(trimmed)
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Extract a string field from a verification block or direct payload field.
fn extract_string_field(payload: &Value, field: &str) -> Option<String> {
    // Try payload.verification.field
    if let Some(v) = payload.get("verification").and_then(|vb| vb.get(field)) {
        if let Some(s) = v.as_str() {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    // Try direct payload.field
    if let Some(v) = payload.get(field) {
        if let Some(s) = v.as_str() {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Emit a single ATTRIBUTE-VALUE-STRING element.
fn emit_attribute_value_string<W: IoWrite>(
    writer: &mut Writer<W>,
    attr_def_id: &str,
    value: &str,
) -> anyhow::Result<()> {
    let mut avs = BytesStart::new("ATTRIBUTE-VALUE-STRING");
    avs.push_attribute(("THE-VALUE", value));
    writer
        .write_event(Event::Start(avs))
        .context("write ATTRIBUTE-VALUE-STRING")?;

    writer
        .write_event(Event::Start(BytesStart::new("DEFINITION")))
        .context("write DEFINITION")?;
    writer
        .write_event(Event::Start(BytesStart::new(
            "ATTRIBUTE-DEFINITION-STRING-REF",
        )))
        .context("write ATTRIBUTE-DEFINITION-STRING-REF start")?;
    writer
        .write_event(Event::Text(BytesText::new(attr_def_id)))
        .context("write ATTRIBUTE-DEFINITION-STRING-REF text")?;
    writer
        .write_event(Event::End(BytesEnd::new("ATTRIBUTE-DEFINITION-STRING-REF")))
        .context("write /ATTRIBUTE-DEFINITION-STRING-REF")?;
    writer
        .write_event(Event::End(BytesEnd::new("DEFINITION")))
        .context("write /DEFINITION")?;

    writer
        .write_event(Event::End(BytesEnd::new("ATTRIBUTE-VALUE-STRING")))
        .context("write /ATTRIBUTE-VALUE-STRING")?;

    Ok(())
}

/// Write a newline character to the underlying writer.
fn write_newline<W: IoWrite>(writer: &mut Writer<W>) -> anyhow::Result<()> {
    writer
        .write_event(Event::Text(BytesText::new("\n")))
        .context("write newline")?;
    Ok(())
}

/// Count SPEC-OBJECT elements in the emitted XML (for CLI success message).
fn count_spec_objects(xml: &[u8]) -> usize {
    xml.windows(b"<SPEC-OBJECT ".len())
        .filter(|w| *w == b"<SPEC-OBJECT ")
        .count()
}

/// Count SPEC-RELATION elements in the emitted XML (for CLI success message).
fn count_spec_relations(xml: &[u8]) -> usize {
    xml.windows(b"<SPEC-RELATION ".len())
        .filter(|w| *w == b"<SPEC-RELATION ")
        .count()
}

/// Wrap XML bytes in a `.reqifz` zip archive.
///
/// Uses fixed FileOptions so the archive is byte-reproducible (D-18).
/// The single entry is named `model.reqif`.
fn wrap_in_reqifz(xml_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));

        // Fixed options for reproducibility (D-18).
        //
        // CR-03: SimpleFileOptions::default() leaves the entry's last-modified
        // time at the zip crate's default (current local time), which makes the
        // archive bytes differ run-to-run / machine-to-machine and silently
        // breaks the D-18 byte-reproducibility contract for the .reqifz artifact.
        // Pin the modification time to the same fixed "DEAL build epoch" used for
        // the XML CREATION-TIME header (FIXED_CREATION_TIME = 2026-01-01T00:00:00).
        let fixed_mtime = zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0)
            .expect("fixed reqifz timestamp 2026-01-01T00:00:00 is valid");
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(fixed_mtime)
            .unix_permissions(0o644);

        zip.start_file("model.reqif", options)
            .context("start model.reqif entry in zip")?;
        zip.write_all(xml_bytes)
            .context("write XML bytes into zip")?;
        zip.finish().context("finish zip archive")?;
    }
    Ok(buf)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_req_ir() -> Value {
        serde_json::json!({
            "edges": [],
            "elements": {
                "gold.reqif01.req.REQ_CHARGE_TIME": {
                    "kind": "requirement_def",
                    "payload": {
                        "direction": "none",
                        "modifiers": [],
                        "name": "REQ_CHARGE_TIME",
                        "doc_comment": "/** The battery shall charge in 30 minutes. */"
                    },
                    "source_file": "01-requirement-def.deal",
                    "span": [0, 100]
                }
            },
            "ir_version": "v0",
            "v": 1
        })
    }

    fn req_with_trace_ir() -> Value {
        serde_json::json!({
            "edges": [
                {
                    "src": "gold.reqif02.trace.Battery",
                    "dst": "gold.reqif02.trace.REQ_CAPACITY",
                    "kind": "satisfies"
                }
            ],
            "elements": {
                "gold.reqif02.trace.REQ_RANGE": {
                    "kind": "requirement_def",
                    "payload": {
                        "direction": "none",
                        "modifiers": [],
                        "name": "REQ_RANGE",
                        "doc_comment": "/** The vehicle shall achieve 300 km range. */"
                    },
                    "source_file": "02-trace-relation.deal",
                    "span": [0, 50]
                },
                "gold.reqif02.trace.REQ_CAPACITY": {
                    "kind": "requirement_def",
                    "payload": {
                        "direction": "none",
                        "modifiers": [],
                        "name": "REQ_CAPACITY",
                        "doc_comment": "/** The battery shall provide 85 kWh. */"
                    },
                    "source_file": "02-trace-relation.deal",
                    "span": [50, 100]
                },
                "gold.reqif02.trace.Battery": {
                    "kind": "part_def",
                    "payload": {
                        "direction": "none",
                        "modifiers": [],
                        "name": "Battery"
                    },
                    "source_file": "02-trace-relation.deal",
                    "span": [100, 150]
                }
            },
            "ir_version": "v0",
            "v": 1
        })
    }

    #[test]
    fn reqif_id_is_deterministic() {
        let id1 = deal_id_to_reqif_id("requirements.system.REQ_BAT_001");
        let id2 = deal_id_to_reqif_id("requirements.system.REQ_BAT_001");
        assert_eq!(id1, id2, "ID must be deterministic");
        assert_eq!(id1, "DEAL_requirements_system_REQ_BAT_001");
        assert!(!id1.contains('.'), "ReqIF ID must not contain dots");
    }

    #[test]
    fn reqif_id_replaces_dots_with_underscores() {
        assert_eq!(deal_id_to_reqif_id("a.b.c.D"), "DEAL_a_b_c_D");
        assert_eq!(deal_id_to_reqif_id("single"), "DEAL_single");
    }

    #[test]
    fn emit_minimal_requirement_ir() {
        let ir = minimal_req_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");

        assert!(xml.contains("<REQ-IF"), "must contain REQ-IF root");
        assert!(xml.contains("SPEC-OBJECT"), "must contain SPEC-OBJECT");
        assert!(
            xml.contains("DEAL_gold_reqif01_req_REQ_CHARGE_TIME"),
            "must contain derived DEAL ID"
        );
        assert!(xml.contains("REQ_CHARGE_TIME"), "must contain LONG-NAME");
    }

    #[test]
    fn emit_skips_part_def() {
        // D-59: part_def must NOT produce a SPEC-OBJECT.
        // The Battery part_def must not appear in the SPEC-OBJECTS section.
        let ir = req_with_trace_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");

        // Extract just the SPEC-OBJECTS section to check only SpecObject elements.
        let spec_objects_section = xml
            .find("<SPEC-OBJECTS>")
            .and_then(|start| xml.find("</SPEC-OBJECTS>").map(|end| &xml[start..end]))
            .unwrap_or("");

        // Battery is a part_def — must NOT appear as a SPEC-OBJECT IDENTIFIER.
        assert!(
            !spec_objects_section.contains("DEAL_gold_reqif02_trace_Battery"),
            "part_def Battery must not appear in SPEC-OBJECTS section (D-59)"
        );

        // Requirements must appear in SPEC-OBJECTS section.
        assert!(
            spec_objects_section.contains("DEAL_gold_reqif02_trace_REQ_RANGE"),
            "REQ_RANGE must be a SPEC-OBJECT"
        );
        assert!(
            spec_objects_section.contains("DEAL_gold_reqif02_trace_REQ_CAPACITY"),
            "REQ_CAPACITY must be a SPEC-OBJECT"
        );
    }

    #[test]
    fn emit_trace_edge_between_two_requirements_produces_spec_relation() {
        // CR-02: a SpecRelation is emitted only when BOTH endpoints are
        // requirement/need nodes, so both SPEC-OBJECT-REF targets resolve to
        // declared SpecObjects (no dangling references).
        let ir = serde_json::json!({
            "edges": [
                {
                    "src": "gold.reqif02.trace.REQ_RANGE",
                    "dst": "gold.reqif02.trace.REQ_CAPACITY",
                    "kind": "satisfies"
                }
            ],
            "elements": {
                "gold.reqif02.trace.REQ_RANGE": {
                    "kind": "requirement_def",
                    "payload": { "name": "REQ_RANGE", "modifiers": [] },
                    "source_file": "02-trace-relation.deal",
                    "span": [0, 50]
                },
                "gold.reqif02.trace.REQ_CAPACITY": {
                    "kind": "requirement_def",
                    "payload": { "name": "REQ_CAPACITY", "modifiers": [] },
                    "source_file": "02-trace-relation.deal",
                    "span": [50, 100]
                }
            },
            "ir_version": "v0",
            "v": 1
        });
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");

        assert!(
            xml.contains("<SPEC-RELATION"),
            "req→req satisfies edge must produce SPEC-RELATION"
        );
        // Both endpoints are requirements, so both refs resolve.
        assert!(xml.contains("DEAL_gold_reqif02_trace_REQ_RANGE"));
        assert!(xml.contains("DEAL_gold_reqif02_trace_REQ_CAPACITY"));

        // CR-02: the emitted document must pass referential-integrity validation
        // (every SPEC-OBJECT-REF resolves to a declared SPEC-OBJECT).
        reqif_schema::validate_reqif_xml(xml.as_bytes())
            .expect("emitted XML with a relation must pass referential-integrity validation");
    }

    #[test]
    fn emit_skips_relation_with_non_requirement_endpoint() {
        // CR-02: a satisfies edge whose source is a part_def (filtered out of
        // SPEC-OBJECTS by D-59) must NOT produce a SpecRelation, because that
        // would create a dangling SPEC-OBJECT-REF to the part_def.
        let ir = req_with_trace_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");

        // No SPEC-RELATION element should be emitted for the Battery→REQ_CAPACITY edge.
        assert!(
            !xml.contains("<SPEC-RELATION "),
            "part_def→req edge must NOT produce a SPEC-RELATION (CR-02 dangling-ref guard): {xml}"
        );

        // The emitted document must still pass referential-integrity validation.
        reqif_schema::validate_reqif_xml(xml.as_bytes())
            .expect("emitted XML must pass referential-integrity validation");
    }

    #[test]
    fn emit_is_byte_deterministic() {
        let ir = minimal_req_ir();
        let xml1 = emit(&ir).expect("first emit failed");
        let xml2 = emit(&ir).expect("second emit failed");
        assert_eq!(xml1, xml2, "emit must be byte-deterministic (D-18)");
    }

    #[test]
    fn emit_xml_passes_structural_validation() {
        let ir = minimal_req_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let result = reqif_schema::validate_reqif_xml(&xml_bytes);
        assert!(
            result.is_ok(),
            "emitted XML must pass structural validation: {:?}",
            result.err()
        );
    }

    #[test]
    fn emit_reqifz_is_valid_zip() {
        let ir = minimal_req_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let reqifz = wrap_in_reqifz(&xml_bytes).expect("wrap_in_reqifz failed");

        let archive = zip::ZipArchive::new(Cursor::new(&reqifz));
        assert!(archive.is_ok(), "reqifz must be a valid zip archive");

        let mut archive = archive.unwrap();
        assert!(!archive.is_empty(), "zip must not be empty");

        // Find .reqif entry.
        let mut found = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            if entry.name().ends_with(".reqif") {
                found = true;
            }
        }
        assert!(found, "reqifz must contain a .reqif entry");
    }

    #[test]
    fn reqifz_archive_is_byte_reproducible() {
        // CR-03: the .reqifz archive must be byte-identical across calls. With the
        // default zip timestamp this failed because the entry stamped the current
        // local time; pinning last_modified_time makes the bytes stable (D-18).
        let ir = minimal_req_ir();
        let xml = emit(&ir).expect("emit failed");
        let archive1 = wrap_in_reqifz(&xml).expect("first wrap_in_reqifz failed");
        let archive2 = wrap_in_reqifz(&xml).expect("second wrap_in_reqifz failed");
        assert_eq!(
            archive1, archive2,
            "wrap_in_reqifz must produce byte-identical archives (CR-03 / D-18)"
        );
    }

    #[test]
    fn emit_sorts_spec_objects_deterministically() {
        // Two requirements — emitted in ID-sorted order regardless of insertion order.
        let ir = req_with_trace_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");

        // REQ_CAPACITY < REQ_RANGE alphabetically.
        let pos_capacity = xml.find("REQ_CAPACITY").unwrap_or(usize::MAX);
        let pos_range = xml.find("REQ_RANGE").unwrap_or(usize::MAX);
        assert!(
            pos_capacity < pos_range,
            "REQ_CAPACITY must appear before REQ_RANGE (D-18 sort)"
        );
    }

    #[test]
    fn doc_comment_text_extracted() {
        // When the payload carries a doc_comment field, it must be used as ReqText.
        let ir = minimal_req_ir();
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");
        // The doc comment text should appear as the ReqText attribute value.
        assert!(
            xml.contains("The battery shall charge in 30 minutes."),
            "doc comment must be extracted as ReqText when present: {xml}"
        );
    }

    #[test]
    fn req_text_falls_back_to_name_when_no_doc_comment() {
        // D-25: IR is comment-free — when no doc_comment in payload, use name.
        let ir = serde_json::json!({
            "edges": [],
            "elements": {
                "pkg.REQ_001": {
                    "kind": "requirement_def",
                    "payload": {
                        "name": "REQ_001",
                        "modifiers": []
                    },
                    "source_file": "f.deal",
                    "span": [0, 10]
                }
            },
            "ir_version": "v0",
            "v": 1
        });
        let xml_bytes = emit(&ir).expect("emit failed");
        let xml = String::from_utf8(xml_bytes).expect("valid UTF-8");
        // The name "REQ_001" should appear as the ReqText value.
        assert!(
            xml.contains("REQ_001"),
            "name must be used as ReqText fallback (D-25): {xml}"
        );
    }
}
