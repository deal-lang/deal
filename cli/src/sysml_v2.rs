//! SysML v2 JSON emitter.
//!
//! Consumes a DEAL IR v0 JSON document (produced by `deal_ir_json` C ABI)
//! and emits a SysML v2 JSON document per the RESEARCH §Pattern 8 mapping
//! table and the OMG SysML 20250201 schema.
//!
//! Key design decisions:
//!   D-19: Rust CLI crate owns this emitter; walks IR via serde_json::Value.
//!   D-24: Single workspace-wide output — one top-level Package containing all elements.
//!   D-18: Alphabetical key order — serde_json::Map uses BTreeMap by default.
//!   Pitfall 5: @id = elementId = UUID v5(SYSML_NAMESPACE, qualified_path).
//!   D-25: IR is comment-free; this emitter makes no doc-comment assumption.
//!   CONTEXT discretion: agent_metadata + simulation_bindings are IGNORED in Phase 2.
//!   CONTEXT discretion: doc-comment → documentation lowering DEFERRED to follow-up ADR.

use std::collections::HashMap;

use anyhow::{anyhow, Context as _};
use serde_json::{json, Map, Value};
use uuid::Uuid;

// ─── UUID synthesis ────────────────────────────────────────────────────────────

/// SysML v2 UUID namespace for DEAL qualified-path synthesis (Pitfall 5).
///
/// This constant MUST NEVER CHANGE across releases — changing it shifts every
/// @id value in every SysML document produced by this emitter, breaking
/// round-trips with any viewer that persisted those IDs (T-02-23).
///
/// Value: the custom DEAL namespace UUID pinned in RESEARCH §Example 3.
const SYSML_NAMESPACE: Uuid = Uuid::from_u128(0x0e72_4d54_7e1a_4b32_9e8c_8e62_2d8f_1234);

/// Synthesize a deterministic UUID v5 from a DEAL qualified path.
///
/// Same qualified_path → same UUID across runs and machines (golden-fixture stability).
pub fn deal_id_to_uuid(qualified_path: &str) -> String {
    Uuid::new_v5(&SYSML_NAMESPACE, qualified_path.as_bytes()).to_string()
}

/// Translate a DEAL dot-path to a SysML v2 qualifiedName (`.` → `::`).
///
/// Per RESEARCH §Pattern 4 line 469.
pub fn deal_id_to_qualified_name(qualified_path: &str) -> String {
    qualified_path.replace('.', "::")
}

/// Extract the simple (unqualified) name from a dot-path.
///
/// `vehicle.battery.BatteryCell` → `BatteryCell`
fn local_name(qualified_path: &str) -> &str {
    qualified_path.rsplit('.').next().unwrap_or(qualified_path)
}

// ─── IR types ─────────────────────────────────────────────────────────────────

/// Deserialized IR node (from the `elements` map in the IR JSON envelope).
#[derive(Debug)]
struct IrNode<'a> {
    /// Fully-qualified path ID (D-23).
    id: &'a str,
    /// NodeKind string (e.g. "part_def", "package", "port_usage").
    kind: &'a str,
    /// Payload object containing name, type_ref, direction, modifiers, etc.
    payload: &'a Value,
}

/// Deserialized IR edge.
#[derive(Debug)]
struct IrEdge<'a> {
    src: &'a str,
    dst: &'a str,
    kind: &'a str,
    /// Behavioral edge payload (IR v0.1): "entry"|"do"|"exit" on a `subaction`
    /// edge; None otherwise.
    subaction_kind: Option<&'a str>,
}

// ─── Top-level emitter ─────────────────────────────────────────────────────────

/// Emit a SysML v2 JSON document from an IR v0 JSON document.
///
/// The IR JSON must conform to `spec/ir/v0/schema.json`. The returned Value
/// is a single top-level Package containing all workspace elements, per D-24.
///
/// Alphabetical key order is maintained automatically because `serde_json::Map`
/// is backed by `BTreeMap` when the `preserve_order` feature is absent (D-18).
pub fn emit(ir_json: &Value) -> anyhow::Result<Value> {
    // Validate basic IR envelope shape.
    let elements_map = ir_json
        .get("elements")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("IR JSON missing 'elements' object"))?;

    let edges_arr = ir_json
        .get("edges")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("IR JSON missing 'edges' array"))?;

    // Parse edges into typed structs.
    let mut edges: Vec<IrEdge> = Vec::with_capacity(edges_arr.len());
    for edge_val in edges_arr {
        let src = edge_val["src"].as_str().unwrap_or("");
        let dst = edge_val["dst"].as_str().unwrap_or("");
        let kind = edge_val["kind"].as_str().unwrap_or("");
        let subaction_kind = edge_val.get("subaction_kind").and_then(|v| v.as_str());
        if !src.is_empty() && !dst.is_empty() && !kind.is_empty() {
            edges.push(IrEdge { src, dst, kind, subaction_kind });
        }
    }

    // Build a map from src_id → vec of outgoing edges for O(1) lookup per element.
    let mut outgoing: HashMap<&str, Vec<&IrEdge>> = HashMap::new();
    for edge in &edges {
        outgoing.entry(edge.src).or_default().push(edge);
    }

    // Parse nodes.
    let mut nodes: Vec<IrNode> = Vec::with_capacity(elements_map.len());
    for (id, node_val) in elements_map {
        let kind = node_val["kind"].as_str().unwrap_or("");
        let payload = &node_val["payload"];
        nodes.push(IrNode { id: id.as_str(), kind, payload });
    }

    // Sort nodes by id for deterministic output (alphabetical element order).
    nodes.sort_by_key(|n| n.id);

    // Build a lookup from id → IrNode for children resolution.
    let node_map: HashMap<&str, &IrNode> = nodes.iter().map(|n| (n.id, n)).collect();

    // Determine which nodes are "top-level" (no parent package in the IR).
    // A node is top-level if it has no incoming 'contains' edge.
    let mut has_parent: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for edge in &edges {
        if edge.kind == "contains" {
            has_parent.insert(edge.dst);
        }
    }

    // Emit all elements into a flat list (SysML model is flat; containment is
    // expressed via ownedRelationship references, not by nesting JSON objects).
    // D-24: single workspace-wide Package at the top level.
    let mut all_sysml_elements: Vec<Value> = Vec::new();

    for node in &nodes {
        let sysml_elem = emit_node(node, &outgoing, &node_map, &edges)?;
        all_sysml_elements.push(sysml_elem);
    }

    // S2.6b: materialize behavioral edges (succession/item_flow/binding) as
    // ConnectorAsUsage elements. Edges are already in deterministic (src,dst,
    // kind) order from the IR emitter. Non-behavioral edge kinds yield None.
    for edge in &edges {
        if let Some(elem) = emit_behavioral_edge(edge) {
            all_sysml_elements.push(elem);
        }
    }

    // Build the top-level workspace Package (D-24).
    // Use a fixed workspace name: "Workspace".
    let workspace_id = "workspace";
    let workspace_uuid = deal_id_to_uuid(workspace_id);

    let mut pkg = Map::new();
    pkg.insert("@id".to_string(), json!(workspace_uuid));
    pkg.insert("@type".to_string(), json!("Package"));
    pkg.insert("declaredName".to_string(), json!("Workspace"));
    pkg.insert("elementId".to_string(), json!(workspace_uuid));
    pkg.insert("ownedRelationship".to_string(), json!(all_sysml_elements));
    pkg.insert("qualifiedName".to_string(), json!("Workspace"));

    Ok(Value::Object(pkg))
}

// ─── Node dispatch ─────────────────────────────────────────────────────────────

/// Emit a single IR node as a SysML v2 JSON object.
///
/// Dispatches to per-kind emitters based on the `kind` string.
/// Unknown kinds are emitted as a generic Element with a `dealKind` extension field.
fn emit_node<'a>(
    node: &IrNode<'a>,
    outgoing: &HashMap<&str, Vec<&IrEdge>>,
    node_map: &HashMap<&str, &IrNode>,
    all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    match node.kind {
        "package" => emit_package(node, outgoing, node_map, all_edges),
        // actor_def → SysML PartDefinition. KerML has no ActorDefinition; an
        // actor is a part carrying an «actor» MetadataFeature. The «actor»
        // marker is part of the (currently deferred) metadata layer, so the
        // structural mapping is PartDefinition — same emitter as part_def.
        "part_def" | "actor_def" => emit_part_def(node, outgoing, node_map, all_edges),
        "port_def" => emit_port_def(node, outgoing, node_map, all_edges),
        "port_usage" => emit_port_usage(node),
        "part_usage" => emit_part_usage(node),
        "attribute_usage" | "attribute_def" => emit_attribute_usage(node),
        "requirement_def" => emit_requirement_def(node, outgoing, node_map, all_edges),
        "traceability_block" => emit_traceability_block(node, outgoing, node_map, all_edges),
        "connect" => emit_connection_usage(node, outgoing, all_edges),
        "expose" => emit_exposed_feature(node),
        "satisfy" => emit_satisfy_dependency(node),
        "need_def" => emit_requirement_def(node, outgoing, node_map, all_edges),
        "interface_def" => emit_interface_def(node, outgoing, node_map, all_edges),
        "connection_def" => emit_connection_def(node),
        "flow_def" | "item_def" => emit_item_def(node),
        // ── Behavioral definitions (IR v0.1, S2.6) ──────────────────────────
        "action_def" => emit_typed_with_members(node, "ActionDefinition", outgoing, node_map), // 8.3.17.3
        "state_def" => emit_state(node, "StateDefinition", outgoing, node_map), // 8.3.18.5
        // ── Behavioral nodes (IR v0.1, S2.6) — metaclass + clause per
        //    behavioral-mapping.md §1 (wiki-verified). ActionUsage closure
        //    8.3.17.4 anchors the family emit shape.
        "action_usage" => emit_typed_with_members(node, "ActionUsage", outgoing, node_map), // 8.3.17.4
        "control_node" => emit_typed_with_members(node, "ActionUsage", outgoing, node_map), // stdlib control (start/done)
        "terminate_action" => emit_typed_with_members(node, "TerminateActionUsage", outgoing, node_map), // 8.3.17.16
        "send_action" => emit_typed_with_members(node, "SendActionUsage", outgoing, node_map), // 8.3.17.15
        "accept_action" => emit_typed_with_members(node, "AcceptActionUsage", outgoing, node_map), // 8.3.17.2
        "assign_action" => emit_typed_with_members(node, "AssignmentActionUsage", outgoing, node_map), // 8.3.17.5
        "perform_action" => emit_typed_with_members(node, "PerformActionUsage", outgoing, node_map), // 8.3.17.14
        "while_loop_action" => emit_typed_with_members(node, "WhileLoopActionUsage", outgoing, node_map), // 8.3.17.19
        "for_loop_action" => emit_typed_with_members(node, "ForLoopActionUsage", outgoing, node_map), // 8.3.17.9
        "decision_node" => emit_typed_with_members(node, "DecisionNode", outgoing, node_map), // 8.3.17.7
        "merge_node" => emit_typed_with_members(node, "MergeNode", outgoing, node_map), // 8.3.17.13
        "fork_node" => emit_typed_with_members(node, "ForkNode", outgoing, node_map), // 8.3.17.8
        "join_node" => emit_typed_with_members(node, "JoinNode", outgoing, node_map), // 8.3.17.11
        "state_usage" => emit_state(node, "StateUsage", outgoing, node_map), // 8.3.18.6
        "transition" => emit_transition(node), // 8.3.18.9 (TransitionUsage)
        "pin" => emit_pin(node), // 8.3.6.3 (ReferenceUsage + FeatureDirectionKind)
        // ── Expression surface (IR v0.2, S3.3) — KerML Expression metaclasses;
        //    SysML inherits them. Clause cited per arm, derived slots untouched,
        //    8.4 implied relationships injected. See expression-mapping.md §1.
        "operator_expr" => emit_operator_expr(node, outgoing, node_map), // KerML 8.3.4.8.17 / FeatureChain 8.3.4.8.4
        "feature_ref_expr" => emit_feature_ref_expr(node), // KerML 8.3.4.8.5 / 8.3.4.8.4
        "literal_expr" => emit_literal_expr(node), // KerML 8.3.4.8.9/.12/.13/.14
        "invocation_expr" => emit_invocation_expr(node, outgoing, node_map), // KerML 8.3.4.8.8 / SysML 8.3.17.17
        "use_case_def"
        | "allocation_def" | "allocate" | "annotation" | "validate" | "subsystem"
        | "system" => emit_generic_element(node),
        "constraint_def" => emit_constraint_def(node),
        "calc_def" => emit_calc_def(node, outgoing, node_map, all_edges),
        _ => emit_generic_element(node),
    }
}

// ─── Shared helpers ────────────────────────────────────────────────────────────

/// Build the base fields common to all SysML elements:
/// `@id`, `@type`, `declaredName`, `elementId`, `qualifiedName`.
fn base_fields(id: &str, sysml_type: &str) -> Map<String, Value> {
    let uuid = deal_id_to_uuid(id);
    let mut m = Map::new();
    m.insert("@id".to_string(), json!(uuid));
    m.insert("@type".to_string(), json!(sysml_type));
    m.insert("declaredName".to_string(), json!(local_name(id)));
    m.insert("elementId".to_string(), json!(uuid));
    m.insert("qualifiedName".to_string(), json!(deal_id_to_qualified_name(id)));
    m
}

/// Collect all members (via 'contains' edges) of a parent node as Identified
/// references `{"@id": "<uuid>"}`.
fn owned_member_refs<'a>(
    parent_id: &str,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
) -> Vec<Value> {
    let mut refs: Vec<Value> = Vec::new();
    if let Some(edges) = outgoing.get(parent_id) {
        let mut member_ids: Vec<&str> = edges
            .iter()
            .filter(|e| e.kind == "contains")
            .map(|e| e.dst)
            .collect();
        member_ids.sort(); // deterministic order
        for member_id in member_ids {
            if node_map.contains_key(member_id) {
                refs.push(json!({ "@id": deal_id_to_uuid(member_id) }));
            }
        }
    }
    refs
}

/// Build `ownedRelationship` for specialization/redefines/subsets edges from `src`.
fn specialization_relationships(
    src_id: &str,
    outgoing: &HashMap<&str, Vec<&IrEdge>>,
) -> Vec<Value> {
    let mut rels: Vec<Value> = Vec::new();
    if let Some(edges) = outgoing.get(src_id) {
        for edge in edges.iter() {
            let rel_type = match edge.kind {
                "specializes" => "Specialization",
                "redefines" => "Redefinition",
                "subsets" => "Subsetting",
                _ => continue,
            };
            let rel_uuid = deal_id_to_uuid(&format!("{src_id}--{rel_type}--{}", edge.dst));
            let general_uuid = deal_id_to_uuid(edge.dst);
            let specific_uuid = deal_id_to_uuid(src_id);
            let mut rel = Map::new();
            rel.insert("@id".to_string(), json!(rel_uuid));
            rel.insert("@type".to_string(), json!(rel_type));
            rel.insert("elementId".to_string(), json!(rel_uuid));
            rel.insert("general".to_string(), json!([{ "@id": general_uuid }]));
            rel.insert("specific".to_string(), json!({ "@id": specific_uuid }));
            rels.push(Value::Object(rel));
        }
    }
    rels
}

/// Generic typed element with FeatureMembership-wrapped owned members.
///
/// Used for the IR v0.1 behavioral nodes (S2.6). Each maps to exactly one SysML
/// v2 metaclass (clause cited at the dispatch site, per
/// spec/ir/v0.1/behavioral-mapping.md §1, wiki-verified via the
/// sysml-v2-wiki-navigator closure). We fill only base + structural-containment
/// slots; the `actionDefinition`/`stateDefinition` attributes are DERIVED and the
/// `Actions::actions`/`States::stateActions` library specializations are
/// normalizer-injected-but-derived (closure 8.3.17.4 checkActionUsageSpecialization;
/// mapping contract §4.5) — neither is authored here.
fn emit_typed_with_members<'a>(
    node: &IrNode,
    sysml_type: &str,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, sysml_type);
    let member_refs = owned_member_refs(node.id, outgoing, node_map);
    let mut owned_rels = specialization_relationships(node.id, outgoing);
    for member_ref in &member_refs {
        let fm_uuid = deal_id_to_uuid(&format!(
            "{}--FeatureMembership--{}",
            node.id,
            member_ref["@id"].as_str().unwrap_or("")
        ));
        let mut fm = Map::new();
        fm.insert("@id".to_string(), json!(fm_uuid));
        fm.insert("@type".to_string(), json!("FeatureMembership"));
        fm.insert("elementId".to_string(), json!(fm_uuid));
        fm.insert("ownedRelatedElement".to_string(), json!([member_ref]));
        owned_rels.push(Value::Object(fm));
    }
    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// Materialize a behavioral IR edge (S2.6b) as a SysML ConnectorAsUsage element.
///
/// succession → SuccessionAsUsage (8.3.13.6), item_flow → FlowUsage (8.3.16.3),
/// binding → BindingConnectorAsUsage (KerML BindingConnector 8.3.4.5.2). The
/// SuccessionAsUsage closure (8.3.13.6: SuccessionAsUsage ▸ Succession ▸
/// ConnectorAsUsage ▸ Connector) establishes that these *AsUsage connectors
/// carry two ConnectorEnds (Connector::connectorEnd). The library
/// specializations (Links::links, Occurrences::happensBeforeLinks) are derived —
/// not authored (§4.5). Returns None for non-behavioral edge kinds (contains/
/// specializes/etc., handled inside emit_node). The `subaction` membership and
/// succession guard expressions are emitted in S2.6c.
fn emit_behavioral_edge(edge: &IrEdge) -> Option<Value> {
    let (sysml_type, tag): (&str, &str) = match edge.kind {
        "succession" => ("SuccessionAsUsage", "succession"),
        "item_flow" => ("FlowUsage", "item_flow"),
        "binding" => ("BindingConnectorAsUsage", "binding"),
        _ => return None,
    };
    let conn_id = format!("{}--{}--{}", edge.src, tag, edge.dst);
    Some(make_connector(&conn_id, sysml_type, edge.src, edge.dst))
}

/// Build a binary ConnectorAsUsage element with two ConnectorEnds (source,
/// target). Shared by behavioral edges (S2.6b) and the owned Succession of a
/// TransitionUsage (S2.6c). Connector shape per the SuccessionAsUsage closure
/// (8.3.13.6: ConnectorAsUsage ▸ Connector::connectorEnd).
fn make_connector(conn_id: &str, sysml_type: &str, src_id: &str, dst_id: &str) -> Value {
    let mk_end = |which: &str, ref_id: &str| -> Value {
        let end_uuid = deal_id_to_uuid(&format!("{conn_id}--end--{which}"));
        json!({
            "@id": end_uuid,
            "@type": "ConnectorEnd",
            "elementId": end_uuid,
            "reference": [{ "@id": deal_id_to_uuid(ref_id) }],
        })
    };
    let mut m = base_fields(conn_id, sysml_type);
    m.insert(
        "connectorEnd".to_string(),
        json!([mk_end("source", src_id), mk_end("target", dst_id)]),
    );
    m.insert("ownedRelationship".to_string(), json!([]));
    Value::Object(m)
}

/// transition → SysML TransitionUsage (8.3.18.9). Per the closure, ALL of
/// source/target/triggerAction/guardExpression/effectAction/succession are
/// DERIVED (computed by the receiving tool from owned features) — none are
/// authored (rule 6). We author the non-derived backbone: the owned Succession
/// (source→target) plus the trigger/guard/effect TransitionFeatureMemberships
/// (8.3.18.8, `kind` authored, `transitionFeature` derived). IR v0.2 (S3.3)
/// completes the triad: guard → a Boolean-valued Expression (the lowered
/// guard_expr node), effect → the lowered invocation_expr node. The derived
/// guardExpression/effectAction attributes are computed by the tool from the
/// kind=guard / kind=effect memberships (8.3.18.9 deriveTransitionUsage*).
fn emit_transition(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "TransitionUsage");
    let mut owned_rels: Vec<Value> = Vec::new();

    let src = node.payload.get("source_ref").and_then(|v| v.as_str());
    let tgt = node.payload.get("target_ref").and_then(|v| v.as_str());
    if let (Some(src), Some(tgt)) = (src, tgt) {
        let succ_id = format!("{}--succession", node.id);
        let succ = make_connector(&succ_id, "SuccessionAsUsage", src, tgt);
        let fm_uuid = deal_id_to_uuid(&format!("{}--FeatureMembership--succession", node.id));
        owned_rels.push(json!({
            "@id": fm_uuid,
            "@type": "FeatureMembership",
            "elementId": fm_uuid,
            "ownedRelatedElement": [succ],
        }));
    }

    // trigger: trigger_ref is a signal/event name → a synthesized AcceptActionUsage
    // (8.3.18.8 validateTransitionFeatureMembershipTriggerAction).
    if let Some(trig) = node.payload.get("trigger_ref").and_then(|v| v.as_str()) {
        if !trig.is_empty() {
            owned_rels.push(transition_feature_membership(node.id, "trigger", "AcceptActionUsage", trig));
        }
    }
    // guard: guard_expr is the id of the lowered Boolean-valued Expression node
    // (operator/feature/literal). kind=guard TransitionFeatureMembership owning
    // it (8.3.18.8 validateTransitionFeatureMembershipGuardExpression); the
    // TransitionUsage.guardExpression is derived from it (8.3.18.9).
    if let Some(g) = node.payload.get("guard_expr").and_then(|v| v.as_str()) {
        if !g.is_empty() {
            owned_rels.push(transition_feature_membership_ref(node.id, "guard", g));
        }
    }
    // effect: effect_ref is the id of the lowered invocation_expr node. kind=effect
    // TransitionFeatureMembership references it; effectAction is derived (8.3.18.9).
    // (Strict effectAction=ActionUsage typing — wrapping the invocation in a
    // PerformActionUsage — is a follow-on refinement; the invocation Expression
    // is structurally present and owned.)
    if let Some(eff) = node.payload.get("effect_ref").and_then(|v| v.as_str()) {
        if !eff.is_empty() {
            owned_rels.push(transition_feature_membership_ref(node.id, "effect", eff));
        }
    }

    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// A TransitionFeatureMembership (8.3.18.8) of `kind` ("guard"/"effect") that
/// references an already-emitted top-level element (the lowered expression node)
/// by `@id`. `kind` is authored; `transitionFeature` is derived.
fn transition_feature_membership_ref(transition_id: &str, kind: &str, ref_id: &str) -> Value {
    let fm_uuid = deal_id_to_uuid(&format!("{transition_id}--TransitionFeatureMembership--{kind}"));
    json!({
        "@id": fm_uuid,
        "@type": "TransitionFeatureMembership",
        "elementId": fm_uuid,
        "kind": kind,
        "ownedRelatedElement": [{ "@id": deal_id_to_uuid(ref_id) }],
    })
}

/// A TransitionFeatureMembership (8.3.18.8) of `kind` ("trigger"/"effect")
/// owning a named Step — an AcceptActionUsage for triggers, a PerformActionUsage
/// for effects. `kind` and the owned feature are authored; `transitionFeature`
/// is derived. `ref_text` is the IR trigger/effect text; its leading identifier
/// becomes the feature's declaredName.
fn transition_feature_membership(transition_id: &str, kind: &str, feat_type: &str, ref_text: &str) -> Value {
    let feat_id = format!("{transition_id}--{kind}");
    let feat_uuid = deal_id_to_uuid(&feat_id);
    let name = ref_text.split('(').next().unwrap_or(ref_text).trim();
    let feature = json!({
        "@id": feat_uuid,
        "@type": feat_type,
        "declaredName": name,
        "elementId": feat_uuid,
        "ownedRelationship": [],
        "qualifiedName": deal_id_to_qualified_name(&feat_id),
    });
    let fm_uuid = deal_id_to_uuid(&format!("{transition_id}--TransitionFeatureMembership--{kind}"));
    json!({
        "@id": fm_uuid,
        "@type": "TransitionFeatureMembership",
        "elementId": fm_uuid,
        "kind": kind,
        "ownedRelatedElement": [feature],
    })
}

// ─── Expression emitters (IR v0.2, S3.3) ─────────────────────────────────────
// Each KerML Expression metaclass; clause cited; only non-derived slots filled.
// Operand/argument features are owned via FeatureMembership references to the
// lowered child expression nodes; the library-function ref, result parameter,
// and result BindingConnectors are derived/injected and never authored
// (rule 6 / §5). See spec/ir/v0.2/expression-mapping.md.

/// operator_expr → KerML OperatorExpression (8.3.4.8.17). Authored: the
/// `operator` symbol + the operand argument features (the lowered op0/op1 child
/// expression nodes). The instantiatedType (library Function resolved from the
/// operator), result, and operand ParameterMemberships are derived (§5).
fn emit_operator_expr<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
) -> anyhow::Result<Value> {
    let mut v = emit_typed_with_members(node, "OperatorExpression", outgoing, node_map)?;
    if let Some(op) = node.payload.get("operator").and_then(|x| x.as_str()) {
        if let Some(obj) = v.as_object_mut() {
            obj.insert("operator".to_string(), json!(op));
        }
    }
    Ok(v)
}

/// feature_ref_expr → KerML FeatureReferenceExpression (8.3.4.8.5) for a single
/// segment, or FeatureChainExpression (8.3.4.8.4) for a dotted path. The
/// `referent`/`targetFeature` is derived (the first non-parameter member) and
/// the result BindingConnector is injected — neither authored. The DEAL feature
/// path is recorded in declaredName (resolving it to a SysML element id is a
/// separate concern, out of scope here).
fn emit_feature_ref_expr(node: &IrNode) -> anyhow::Result<Value> {
    let segs: Vec<&str> = node
        .payload
        .get("referent_segments")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
        .unwrap_or_default();
    let sysml_type = if segs.len() >= 2 { "FeatureChainExpression" } else { "FeatureReferenceExpression" };
    let mut m = base_fields(node.id, sysml_type);
    if !segs.is_empty() {
        m.insert("declaredName".to_string(), json!(segs.join(".")));
    }
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// literal_expr → KerML LiteralBoolean/Integer/Rational/String
/// (8.3.4.8.9/.12/.13/.14). Authored: the typed `value`. The result typing
/// (Integer/Rational/Boolean/String) is derived via checkLiteral*Specialization.
fn emit_literal_expr(node: &IrNode) -> anyhow::Result<Value> {
    let kind = node.payload.get("literal_kind").and_then(|v| v.as_str()).unwrap_or("string");
    let raw = node.payload.get("literal_value").and_then(|v| v.as_str()).unwrap_or("");
    let (sysml_type, value): (&str, Value) = match kind {
        "integer" => (
            "LiteralInteger",
            raw.parse::<i64>().map(|n| json!(n)).unwrap_or_else(|_| json!(raw)),
        ),
        "rational" => (
            "LiteralRational",
            raw.parse::<f64>().map(|n| json!(n)).unwrap_or_else(|_| json!(raw)),
        ),
        "boolean" => ("LiteralBoolean", json!(raw == "true")),
        _ => ("LiteralString", json!(raw)),
    };
    let mut m = base_fields(node.id, sysml_type);
    m.insert("value".to_string(), value);
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// call → KerML InvocationExpression (8.3.4.8.8), or SysML
/// TriggerInvocationExpression (8.3.17.17) when a trigger `kind` (when/at/after)
/// is present. Authored: the argument features (lowered argN child nodes) + the
/// trigger `kind`. The instantiatedType (invoked behavior/function resolved from
/// callee_ref), arguments, and result binding are derived (§5).
fn emit_invocation_expr<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
) -> anyhow::Result<Value> {
    let trigger = node.payload.get("trigger_kind").and_then(|v| v.as_str());
    let sysml_type = if trigger.is_some() { "TriggerInvocationExpression" } else { "InvocationExpression" };
    let mut v = emit_typed_with_members(node, sysml_type, outgoing, node_map)?;
    if let Some(obj) = v.as_object_mut() {
        if let Some(k) = trigger {
            obj.insert("kind".to_string(), json!(k));
        }
    }
    Ok(v)
}

/// state_def → StateDefinition (8.3.18.5), state_usage → StateUsage (8.3.18.6).
///
/// Like emit_typed_with_members, except an entry/do/exit subaction member is
/// owned via a StateSubactionMembership (8.3.18.4) carrying its `kind`, rather
/// than a plain FeatureMembership. Per the StateSubactionMembership closure it is
/// a FeatureMembership whose `action` is derived (not authored) and whose `kind`
/// ∈ {entry, do, exit} is authored; owningType must be a State (satisfied here).
fn emit_state<'a>(
    node: &IrNode,
    sysml_type: &str,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, sysml_type);

    // subaction target id → kind, from this state's outgoing subaction edges.
    let mut subaction_kind: HashMap<&str, &str> = HashMap::new();
    if let Some(edges) = outgoing.get(node.id) {
        for e in edges.iter() {
            if e.kind == "subaction" {
                if let Some(k) = e.subaction_kind {
                    subaction_kind.insert(e.dst, k);
                }
            }
        }
    }

    // Owned members via 'contains' edges (sorted, deterministic).
    let mut member_ids: Vec<&str> = Vec::new();
    if let Some(edges) = outgoing.get(node.id) {
        let mut ids: Vec<&str> = edges
            .iter()
            .filter(|e| e.kind == "contains")
            .map(|e| e.dst)
            .collect();
        ids.sort();
        for id in ids {
            if node_map.contains_key(id) {
                member_ids.push(id);
            }
        }
    }

    let mut member_refs: Vec<Value> = Vec::new();
    let mut owned_rels = specialization_relationships(node.id, outgoing);
    for member_id in &member_ids {
        let member_uuid = deal_id_to_uuid(member_id);
        member_refs.push(json!({ "@id": member_uuid }));
        if let Some(kind) = subaction_kind.get(member_id) {
            let ssm_uuid =
                deal_id_to_uuid(&format!("{}--StateSubactionMembership--{}", node.id, member_id));
            owned_rels.push(json!({
                "@id": ssm_uuid,
                "@type": "StateSubactionMembership",
                "elementId": ssm_uuid,
                "kind": kind,
                "ownedRelatedElement": [{ "@id": member_uuid }],
            }));
        } else {
            let fm_uuid =
                deal_id_to_uuid(&format!("{}--FeatureMembership--{}", node.id, member_uuid));
            owned_rels.push(json!({
                "@id": fm_uuid,
                "@type": "FeatureMembership",
                "elementId": fm_uuid,
                "ownedRelatedElement": [{ "@id": member_uuid }],
            }));
        }
    }

    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// pin → SysML ReferenceUsage (8.3.6.3) carrying a FeatureDirectionKind
/// `direction` (Feature::direction, inherited via Usage ▸ Feature). A pin is a
/// directed, non-compositional parameter of an action. Per the ReferenceUsage
/// closure, `isReference` is derived (always true) and not authored. The pin's
/// declared type is emitted as a FeatureTyping (KerML 8.3.3.3.7): a
/// Specialization whose `type` is the declared Type and whose `typedFeature` is
/// the pin (both redefine general/specific; `owningFeature` is derived).
fn emit_pin(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "ReferenceUsage");
    let direction = node.payload["direction"].as_str().unwrap_or("none");
    let sysml_direction = match direction {
        "in" => "in",
        "out" => "out",
        "inout" => "inout",
        _ => "none",
    };
    m.insert("direction".to_string(), json!(sysml_direction));

    let mut owned_rels: Vec<Value> = Vec::new();
    if let Some(type_ref) = node.payload.get("type_ref").and_then(|v| v.as_str()) {
        if !type_ref.is_empty() {
            let ft_uuid = deal_id_to_uuid(&format!("{}--FeatureTyping--{}", node.id, type_ref));
            owned_rels.push(json!({
                "@id": ft_uuid,
                "@type": "FeatureTyping",
                "elementId": ft_uuid,
                "type": { "@id": deal_id_to_uuid(type_ref) },
                "typedFeature": { "@id": deal_id_to_uuid(node.id) },
            }));
        }
    }
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

// ─── Per-kind emitters ─────────────────────────────────────────────────────────

/// Package → SysML `Package`.
fn emit_package<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "Package");
    let members = owned_member_refs(node.id, outgoing, node_map);
    m.insert("ownedMembership".to_string(), json!(members));
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// part_def → SysML `PartDefinition`.
fn emit_part_def<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "PartDefinition");

    // Collect owned member references (via 'contains' edges).
    let member_refs = owned_member_refs(node.id, outgoing, node_map);

    // Build ownedRelationship: specializations + member ownership refs.
    let mut owned_rels = specialization_relationships(node.id, outgoing);
    // Add a FeatureMembership wrapper for each owned member.
    for member_ref in &member_refs {
        let fm_uuid = deal_id_to_uuid(&format!(
            "{}--FeatureMembership--{}",
            node.id,
            member_ref["@id"].as_str().unwrap_or("")
        ));
        let mut fm = Map::new();
        fm.insert("@id".to_string(), json!(fm_uuid));
        fm.insert("@type".to_string(), json!("FeatureMembership"));
        fm.insert("elementId".to_string(), json!(fm_uuid));
        fm.insert("ownedRelatedElement".to_string(), json!([member_ref]));
        owned_rels.push(Value::Object(fm));
    }

    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// port_def → SysML `PortDefinition`.
fn emit_port_def<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "PortDefinition");
    let member_refs = owned_member_refs(node.id, outgoing, node_map);
    let mut owned_rels = specialization_relationships(node.id, outgoing);
    for member_ref in &member_refs {
        let fm_uuid = deal_id_to_uuid(&format!(
            "{}--FeatureMembership--{}",
            node.id,
            member_ref["@id"].as_str().unwrap_or("")
        ));
        let mut fm = Map::new();
        fm.insert("@id".to_string(), json!(fm_uuid));
        fm.insert("@type".to_string(), json!("FeatureMembership"));
        fm.insert("elementId".to_string(), json!(fm_uuid));
        fm.insert("ownedRelatedElement".to_string(), json!([member_ref]));
        owned_rels.push(Value::Object(fm));
    }
    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// interface_def → SysML `InterfaceDefinition`.
fn emit_interface_def<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "InterfaceDefinition");
    let member_refs = owned_member_refs(node.id, outgoing, node_map);
    let owned_rels = specialization_relationships(node.id, outgoing);
    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// port_usage → SysML `PortUsage`.
fn emit_port_usage(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "PortUsage");

    // Direction mapping (RESEARCH §Pattern 8).
    let direction = node.payload["direction"].as_str().unwrap_or("none");
    let sysml_direction = match direction {
        "in" => "in",
        "out" => "out",
        "inout" => "inout",
        _ => "none",
    };
    m.insert("direction".to_string(), json!(sysml_direction));

    // Type reference (maps to portDefinition via Identified ref).
    if let Some(type_ref) = node.payload["type_ref"].as_str() {
        if !type_ref.is_empty() {
            let type_uuid = deal_id_to_uuid(type_ref);
            m.insert("portDefinition".to_string(), json!([{ "@id": type_uuid }]));
        }
    }

    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// part_usage → SysML `PartUsage`.
fn emit_part_usage(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "PartUsage");

    if let Some(type_ref) = node.payload["type_ref"].as_str() {
        if !type_ref.is_empty() {
            let type_uuid = deal_id_to_uuid(type_ref);
            m.insert("partDefinition".to_string(), json!([{ "@id": type_uuid }]));
        }
    }

    // Multiplicity from modifiers if present.
    let modifiers = node.payload["modifiers"].as_array();
    if let Some(mods) = modifiers {
        let is_ordered = mods.iter().any(|m| m.as_str() == Some("ordered"));
        if is_ordered {
            m.insert("isOrdered".to_string(), json!(true));
        }
    }

    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// attribute_usage / attribute_def → SysML `AttributeUsage`.
fn emit_attribute_usage(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "AttributeUsage");

    if let Some(type_ref) = node.payload["type_ref"].as_str() {
        if !type_ref.is_empty() {
            let type_uuid = deal_id_to_uuid(type_ref);
            m.insert("definition".to_string(), json!([{ "@id": type_uuid }]));
        }
    }

    // Direction for derived attributes.
    let direction = node.payload["direction"].as_str().unwrap_or("none");
    if direction != "none" {
        m.insert("direction".to_string(), json!(direction));
    }

    // Modifiers.
    let modifiers = node.payload["modifiers"].as_array();
    if let Some(mods) = modifiers {
        let is_derived = mods.iter().any(|m| m.as_str() == Some("derived"));
        if is_derived {
            m.insert("isDerived".to_string(), json!(true));
        }
        let is_ordered = mods.iter().any(|m| m.as_str() == Some("ordered"));
        if is_ordered {
            m.insert("isOrdered".to_string(), json!(true));
        }
    }

    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// requirement_def / need_def → SysML `RequirementDefinition`.
fn emit_requirement_def<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "RequirementDefinition");
    let member_refs = owned_member_refs(node.id, outgoing, node_map);
    let owned_rels = specialization_relationships(node.id, outgoing);
    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));
    Ok(Value::Object(m))
}

/// traceability_block → SysML `Package` containing `Dependency` elements.
fn emit_traceability_block<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "Package");

    // Collect satisfies edges rooted in this traceability block.
    let mut dep_refs: Vec<Value> = Vec::new();
    for edge in all_edges {
        if edge.kind == "satisfies" && edge.src.starts_with(node.id) {
            let dep_uuid = deal_id_to_uuid(&format!("{}--satisfies--{}", edge.src, edge.dst));
            dep_refs.push(json!({ "@id": dep_uuid }));
        }
    }

    let member_refs = owned_member_refs(node.id, outgoing, node_map);
    let mut all_refs = member_refs;
    all_refs.extend(dep_refs);

    m.insert("ownedMembership".to_string(), json!(all_refs));
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// satisfy → SysML `Dependency` with kind "satisfy"
/// (satisfies edge from @trace per RESEARCH §Pattern 8).
fn emit_satisfy_dependency(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "Dependency");

    // requirement_ref holds the target requirement qualified path.
    if let Some(req_ref) = node.payload["requirement_ref"].as_str() {
        if !req_ref.is_empty() {
            let req_uuid = deal_id_to_uuid(req_ref);
            m.insert("target".to_string(), json!([{ "@id": req_uuid }]));
        }
    }

    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// connect (.dealx) → SysML `ConnectionUsage` with `connectorEnd` array.
fn emit_connection_usage<'a>(
    node: &IrNode,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "ConnectionUsage");

    // Build connectorEnd from connects_via + carries edges.
    let mut connector_ends: Vec<Value> = Vec::new();
    if let Some(edges) = outgoing.get(node.id) {
        for edge in edges.iter() {
            match edge.kind {
                "connects_via" | "carries" => {
                    let end_uuid = deal_id_to_uuid(&format!("{}--end--{}", node.id, edge.dst));
                    let target_uuid = deal_id_to_uuid(edge.dst);
                    let mut end = Map::new();
                    end.insert("@id".to_string(), json!(end_uuid));
                    end.insert("@type".to_string(), json!("ConnectorEnd"));
                    end.insert("elementId".to_string(), json!(end_uuid));
                    end.insert("reference".to_string(), json!([{ "@id": target_uuid }]));
                    connector_ends.push(Value::Object(end));
                }
                _ => {}
            }
        }
    }

    m.insert("connectorEnd".to_string(), json!(connector_ends));
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// expose (.dealx) → SysML `PortUsage` (port re-export).
fn emit_exposed_feature(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "PortUsage");
    m.insert("isComposite".to_string(), json!(false));
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// connection_def → SysML `ConnectionDefinition`.
fn emit_connection_def(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "ConnectionDefinition");
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// flow_def / item_def → SysML `ItemDefinition`.
fn emit_item_def(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "ItemDefinition");
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// calc_def → KerML `Function` (directed params + result feature).
///
/// Maps per design §7 (RESEARCH): calc def → KerML Function with:
///   - directed "in" parameters → ownedMembership via FeatureMembership
///   - return type → result Feature
///   - precision contract → metadata annotation feature (D-08, carried not enforced)
///   - dealKind = "calc_def" for round-trip traceability
///
/// Directed-feature ORDER is preserved (D-12: in-params == sim inputs, out-params == sim outputs).
fn emit_calc_def<'a>(
    node: &IrNode<'a>,
    outgoing: &HashMap<&str, Vec<&IrEdge<'a>>>,
    node_map: &HashMap<&str, &IrNode>,
    _all_edges: &[IrEdge],
) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "Function");
    m.insert("dealKind".to_string(), json!("calc_def"));

    // Collect owned member references (via 'contains' edges).
    let member_refs = owned_member_refs(node.id, outgoing, node_map);

    // Build ownedRelationship: specializations first.
    let owned_rels = specialization_relationships(node.id, outgoing);

    m.insert("ownedMembership".to_string(), json!(member_refs));
    m.insert("ownedRelationship".to_string(), json!(owned_rels));

    // Attach return type as a result feature (from payload.return_type).
    if let Some(return_type) = node.payload["return_type"].as_str() {
        if !return_type.is_empty() {
            let result_uuid = deal_id_to_uuid(&format!("{}--result", node.id));
            let result_type_uuid = deal_id_to_uuid(return_type);
            let mut result_feature = Map::new();
            result_feature.insert("@id".to_string(), json!(result_uuid));
            result_feature.insert("@type".to_string(), json!("Feature"));
            result_feature.insert("direction".to_string(), json!("out"));
            result_feature.insert("elementId".to_string(), json!(result_uuid));
            result_feature.insert("isEnd".to_string(), json!(false));
            result_feature.insert("isResult".to_string(), json!(true));
            result_feature.insert("featureTyping".to_string(), json!([{ "@id": result_type_uuid }]));
            m.insert("resultFeature".to_string(), json!(result_feature));
        }
    }

    // Attach precision as metadata annotation if present (D-08, parse-and-carry).
    if let Some(prec) = node.payload["precision"].as_object() {
        m.insert("metadata".to_string(), json!({ "precision": prec }));
    }

    Ok(Value::Object(m))
}

/// constraint_def → KerML `Predicate` (Boolean Function per design §7).
///
/// constraint_def is a named predicate — it maps to KerML Predicate, which is
/// a subtype of Function restricted to Boolean results. Removed from the
/// emit_generic_element arm (T-05.2-W4-02 mitigation: byte-exact golden asserts Predicate type).
fn emit_constraint_def(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "Predicate");
    m.insert("dealKind".to_string(), json!("constraint_def"));
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

/// Generic fallback for IR kinds not explicitly mapped.
///
/// Uses `@type: "Namespace"` — a permissive SysML base type that satisfies
/// the schema. Adds a `dealKind` extension field for traceability.
fn emit_generic_element(node: &IrNode) -> anyhow::Result<Value> {
    let mut m = base_fields(node.id, "Namespace");
    m.insert("dealKind".to_string(), json!(node.kind));
    m.insert("ownedRelationship".to_string(), json!([]));
    Ok(Value::Object(m))
}

// ─── Public build pipeline helper ─────────────────────────────────────────────

/// Invoke the full IR → SysML v2 pipeline for a single source file.
///
/// Called from `main.rs` `run_build` and from integration tests.
/// Returns the emitted SysML v2 JSON `Value`.
///
/// Pitfall 3 mitigation: `ir_json_bytes` must be Cow-cloned BEFORE the caller
/// calls `deal_free`; this function takes `&[u8]` (already cloned).
pub fn emit_from_bytes(ir_json_bytes: &[u8]) -> anyhow::Result<Value> {
    let ir_value: Value = serde_json::from_slice(ir_json_bytes)
        .with_context(|| "failed to parse IR JSON from deal_ir_json output")?;
    emit(&ir_value)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_is_deterministic() {
        let u1 = deal_id_to_uuid("gold01.parts.Engine");
        let u2 = deal_id_to_uuid("gold01.parts.Engine");
        assert_eq!(u1, u2, "UUID must be deterministic for the same path");
    }

    #[test]
    fn uuid_differs_for_different_paths() {
        let u1 = deal_id_to_uuid("gold01.parts.Engine");
        let u2 = deal_id_to_uuid("gold01.parts.Motor");
        assert_ne!(u1, u2, "Different paths must produce different UUIDs");
    }

    #[test]
    fn uuid_is_valid_format() {
        let u = deal_id_to_uuid("gold01.parts.Engine");
        // Must be parseable as UUID.
        let parsed = Uuid::parse_str(&u);
        assert!(parsed.is_ok(), "UUID must be valid: {}", u);
    }

    #[test]
    fn qualified_name_dot_to_colon() {
        assert_eq!(
            deal_id_to_qualified_name("vehicle.battery.BatteryCell"),
            "vehicle::battery::BatteryCell"
        );
        assert_eq!(deal_id_to_qualified_name("pkg"), "pkg");
    }

    #[test]
    fn emit_minimal_ir() {
        let ir = serde_json::json!({
            "edges": [],
            "elements": {
                "gold01.parts.Engine": {
                    "kind": "part_def",
                    "payload": {
                        "direction": "none",
                        "modifiers": [],
                        "name": "Engine"
                    },
                    "source_file": "gold01.deal",
                    "span": [0, 20]
                }
            },
            "ir_version": "v0",
            "v": 1
        });
        let result = emit(&ir).expect("emit failed");
        assert_eq!(result["@type"], "Package", "top-level must be Package");
        let owned = result["ownedRelationship"].as_array().expect("ownedRelationship array");
        assert_eq!(owned.len(), 1, "should have 1 element");
        assert_eq!(owned[0]["@type"], "PartDefinition");
        // UUID must be consistent.
        let expected_uuid = deal_id_to_uuid("gold01.parts.Engine");
        assert_eq!(owned[0]["@id"], expected_uuid);
        assert_eq!(owned[0]["elementId"], expected_uuid);
        assert_eq!(owned[0]["qualifiedName"], "gold01::parts::Engine");
    }

    #[test]
    fn emit_preserves_alphabetical_keys() {
        let ir = serde_json::json!({
            "edges": [],
            "elements": {
                "pkg.Engine": {
                    "kind": "part_def",
                    "payload": { "direction": "none", "modifiers": [], "name": "Engine" },
                    "source_file": "f.deal",
                    "span": [0, 10]
                }
            },
            "ir_version": "v0",
            "v": 1
        });
        let result = emit(&ir).expect("emit failed");
        let raw = serde_json::to_string(&result).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("reparse");
        let keys: Vec<&str> = parsed.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        let sorted: Vec<&str> = {
            let mut s = keys.clone();
            s.sort();
            s
        };
        assert_eq!(keys, sorted, "Top-level keys must be alphabetical (D-18)");
    }

    #[test]
    fn emit_package_node() {
        let ir = serde_json::json!({
            "edges": [],
            "elements": {
                "mypkg": {
                    "kind": "package",
                    "payload": { "direction": "none", "modifiers": [], "name": "mypkg" },
                    "source_file": "f.deal",
                    "span": [0, 10]
                }
            },
            "ir_version": "v0",
            "v": 1
        });
        let result = emit(&ir).expect("emit failed");
        let owned = result["ownedRelationship"].as_array().unwrap();
        assert_eq!(owned[0]["@type"], "Package");
        assert_eq!(owned[0]["qualifiedName"], "mypkg");
    }

    #[test]
    fn emit_port_usage_with_direction() {
        let ir = serde_json::json!({
            "edges": [],
            "elements": {
                "pkg.Part.myPort": {
                    "kind": "port_usage",
                    "payload": { "direction": "out", "modifiers": [], "name": "myPort" },
                    "source_file": "f.deal",
                    "span": [0, 10]
                }
            },
            "ir_version": "v0",
            "v": 1
        });
        let result = emit(&ir).expect("emit failed");
        let owned = result["ownedRelationship"].as_array().unwrap();
        assert_eq!(owned[0]["@type"], "PortUsage");
        assert_eq!(owned[0]["direction"], "out");
    }
}
