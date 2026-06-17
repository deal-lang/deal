//! AST node model (D-01, D-03, D-15).
//!
//! Plan 03 extends the minimal Wave-0 stub with the full tagged-union
//! AST covering all .deal grammar productions and .dealx placeholder
//! variants (Plan 04 fills comp_* bodies).
//!
//! Decision summary:
//!   D-01: `Node = struct { kind, span, payload: union(NodeKind) }`
//!   D-02: every *Node is arena-allocated; no heap ownership outside arena
//!   D-03: unified NodeKind enum across .deal + .dealx
//!   D-15: Span on every node (byte offsets, extern struct for C ABI)
//!   D-18: JSON payload fields alphabetical (see json.zig)

const std = @import("std");

/// Byte-offset span into the duped source buffer (D-15).
/// `extern struct` required because Diagnostic.span crosses the C ABI when
/// emitted into the diagnostics JSON. Always 8 bytes; trivially copyable.
pub const Span = extern struct {
    start: u32,
    end: u32,
};

/// Root mode marker (D-03) — populated from filename extension (D-12) and
/// surfaced in the AST JSON envelope (D-04) as `"mode":"deal"|"dealx"`.
pub const Mode = enum {
    deal,
    dealx,
};

// ─── Open-tag stack entries (D-07 / Plan 04 parser-owned tag stack) ─────
//
// The `.dealx` parser maintains an open-tag stack so that `[<name>]` opens
// can be matched against their `[</name>]` closes (Pattern 4 / RESEARCH
// lines 383-426). The Parser struct in parser_deal.zig holds the
// `std.ArrayList(OpenTag)` for both modes — it's empty for `.deal` files
// and populated for `.dealx`. Living here avoids a circular import between
// parser_deal.zig and parser_dealx.zig: parser_dealx already imports
// parser_deal (to access the Parser struct), so the reverse direction
// would form a cycle in the module graph.

pub const OpenTag = struct {
    name: []const u8,
    span: Span,
};

/// T-04-01 DoS bound on nested composition tags (RESEARCH line 1006).
/// Past this depth, pushTag emits E0303 and drops the tag; the parser
/// continues without growing the stack further.
pub const MAX_TAG_DEPTH: u32 = 1024;

// ─── NodeKind ──────────────────────────────────────────────────────────────

/// Unified node kind across `.deal` and `.dealx` (D-03).
///
/// .deal variants are fully implemented in Plan 03.
/// .dealx comp_* variants are declared here with placeholder payload bodies
/// so json.zig's exhaustive switch compiles; Plan 04 fills the parser bodies.
pub const NodeKind = enum {
    // ── File-level ──────────────────────────────────────────────────
    deal_file,
    dealx_file,
    header_block,
    package_decl,
    import_decl,
    export_decl,

    // ── Element definitions (14 + 1 new) ────────────────────────────
    part_def,
    port_def,
    action_def,
    state_def,
    attribute_def,
    item_def,
    interface_def,
    connection_def,
    flow_def,
    allocation_def,
    requirement_def,
    constraint_def,
    need_def,
    use_case_def,
    actor_def,          // ActorDefinition (SD-18b — external participant; «actor» on a part/item)

    // ── Calc/constraint definitions (SD-21/22/23 — Phase 05.2 Wave 2) ──
    calc_def,           // CalcDefinition (new top-level + _MemberContent)
    param_list,         // ParameterList (for both calc and constraint params)
    param_decl,         // ParameterDecl (direction + name + type + multiplicity)
    return_contract,    // ReturnContract ("=>" ContractItem+)
    precision_spec,     // PrecisionSpec (sig N | ± Tolerance)
    constraint_ref,     // ConstraintRef (QualifiedName _ArgumentList?)
    constraint_body,    // ConstraintBody ("{" RequireStatement* ... "}")
    calc_body,          // CalcBody ("{" LocalBinding* ReturnStatement "}")
    require_statement,  // RequireStatement ("require" ConditionExpression ";")

    // ── Behavioral surface (BH-1..BH-7, Stage-2 S2.2) ────────────────
    action_body,        // ActionBody / StateBody container (BH-7)
    pin_decl,           // PinDecl (in/out/inout x : T)
    succession_chain,   // SuccessionChain (a -> b -> c)
    control_ref,        // ControlRef (start | done | terminate in flow position)
    decide_block,       // DecideBlock (decide { [g] -> ref })
    par_block,          // ParBlock (par { -> ref })
    loop_statement,     // LoopStatement (loop while/until | for x in e)
    send_action,        // SendAction (send P to T)
    accept_action,      // AcceptAction (accept S [g])
    assign_action,      // AssignAction (assign x := e)
    perform_statement,  // PerformStatement (f(x);)
    item_flow_statement,// ItemFlowStatement (a.x ~> b.y)
    binding_statement,  // BindingStatement (bind a = b)
    escape_node,        // EscapeNode (node a : T)
    escape_succession,  // EscapeSuccession (succession a -> b)
    entry_do_exit,      // EntryDoExit (entry|do|exit / Behavior)
    transition_statement, // TransitionStatement (on T [g] / e -> S)
    target_ref,         // TargetRef (done | terminate | QualifiedName)

    // ── Member / usages (14) ────────────────────────────────────────
    part_usage,
    port_usage,
    attribute_usage,
    action_usage,
    actor_usage,
    subject_usage,
    state_usage,
    item_usage,
    interface_usage,
    connection_usage,
    flow_usage,
    allocation_usage,
    requirement_usage,
    constraint_usage,
    need_usage,
    use_case_usage,

    // ── Structural/type nodes ────────────────────────────────────────
    type_annotation,
    multiplicity,
    modifier_list,
    visibility_wrapper,
    specialization,
    redefinition,
    operator_statement,
    inline_body,
    structural_relationship,

    // ── Annotation nodes ─────────────────────────────────────────────
    annotation,
    doc_comment,
    annotation_body,

    // ── Verification / precondition / postcondition ──────────────────
    verification_block,
    precondition_block,
    postcondition_block,

    // ── Expressions ──────────────────────────────────────────────────
    binary,
    unary,
    member_access,
    call,
    identifier,
    int_literal,
    real_literal,
    string_literal,
    template_literal,
    boolean_literal,
    interpolation,
    /// Array literal `[item, item, ...]` (D-04). Replaces the synthetic
    /// `call(__array__, items)` encoding. Single source of truth for
    /// `[expr, expr, ...]` in both .deal and .dealx contexts.
    array_literal,

    // ── Composition / .dealx (Plan 04 fills bodies) ─────────────────
    system_block,
    subsystem_block,
    component_instance,
    comp_connect,
    expose_tag,
    traceability_block,
    allocate_tag,
    satisfy_block,
    validate_tag,
    /// Inline object literal — `TypeName? { key: value, ... }` produced
    /// inside `via={...}` / `carrying={...}` blocks (D-08). Plan 04
    /// Task 4.1b.
    object_literal,
};

// ─── Payload sub-types ────────────────────────────────────────────────────

/// A single @header field: key/value pair with span.
pub const HeaderField = struct {
    key: []const u8,
    value: []const u8,
    span: Span,
};

pub const ImportItem = struct {
    name: []const u8,
    alias: ?[]const u8,
    /// P2 WS-C0: span of the imported name token, for precise rename of the
    /// `import` statement. Empty when unset.
    name_span: Span = .{ .start = 0, .end = 0 },
};

pub const ImportKind = enum { simple, named, wildcard, alias };

/// Modifier flags collected before an element definition or usage.
pub const Modifier = enum {
    abstract,
    derived,
    readonly,
    ordered,
    nonunique,
    individual,
    variation,
    portion,
    end_kw,
    ref_kw,
};

/// Port/attribute direction.
pub const Direction = enum { none, in, out, inout };

/// Binary operator enumeration (D-01 / RESEARCH lines 343-346).
pub const BinaryOp = enum {
    add,
    sub,
    mul,
    div,
    eq,
    neq,
    lt,
    le,
    gt,
    ge,
    log_and,
    log_or,
};

/// Unary operator enumeration.
pub const UnaryOp = enum { neg, not, bang };

/// Template literal part — either fixed text or an interpolated expression.
pub const TemplatePart = union(enum) {
    text: []const u8,
    expr: *Node,
};

/// Annotation body field: key + expression value.
pub const AnnotationField = struct {
    key: []const u8,
    value: *Node,
    span: Span,
};

/// Verification field kind.
pub const VerifFieldKind = enum {
    accepts,
    rejects,
    threshold,
    operator,
    conditions,
};

/// A single verification field (key + value node).
pub const VerifField = struct {
    kind: VerifFieldKind,
    value: *Node,
    span: Span,
};

// ─── Payload structs ──────────────────────────────────────────────────────

pub const DealFile = struct {
    header: ?*Node,
    package_decl: ?*Node,
    imports: []*Node,
    exports: []*Node,
    definitions: []*Node,
};

pub const DealxFile = struct {
    header: ?*Node,
    package_decl: ?*Node,
    imports: []*Node,
    root_tags: []*Node,
};

pub const HeaderBlock = struct {
    fields: []HeaderField,
};

pub const PackageDecl = struct {
    /// Dot-separated segments e.g. ["vehicle", "battery"]
    segments: [][]const u8,
};

pub const ImportDecl = struct {
    path: [][]const u8,
    kind: ImportKind,
    items: []ImportItem,
};

pub const ExportDecl = struct {
    module: []const u8,
    items: [][]const u8,
};

/// Shared shape for all 14 element definition kinds.
pub const ElementDef = struct {
    name: []const u8,
    /// P2 WS-C0: span of the declared name token (for precise rename of the
    /// declaration). Empty when unset; callers fall back to the node span.
    name_span: Span = .{ .start = 0, .end = 0 },
    modifiers: []Modifier,
    direction: Direction,
    specializes: ?*Node, // structural_relationship node
    annotations: []*Node,
    members: []*Node,
    doc: ?*Node,

    // D-28 / D-29 comment attachment fields (Plan 02-01).
    // Empty slices / null are the default (arena-default zero-init).
    leading_comments: []Comment = &.{},
    trailing_comments: []Comment = &.{},
    doc_comment: ?*DocComment = null,
};

// Each _def kind is an alias for ElementDef — the kind field on Node
// distinguishes them. Zig allows type aliases so the Payload union below
// names them separately without code duplication.
pub const PartDef = ElementDef;
pub const PortDef = ElementDef;
pub const ActionDef = ElementDef;
pub const StateDef = ElementDef;
pub const AttributeDef = ElementDef;
pub const ItemDef = ElementDef;
pub const InterfaceDef = ElementDef;
pub const ConnectionDef = ElementDef;
pub const FlowDef = ElementDef;
pub const AllocationDef = ElementDef;
pub const RequirementDef = ElementDef;
// NOTE: ConstraintDef alias removed — replaced by ConstraintDefinition struct below (Phase 05.2)
pub const NeedDef = ElementDef;
pub const UseCaseDef = ElementDef;
pub const ActorDef = ElementDef;

/// Real ConstraintDefinition struct (replaces the former ConstraintDef = ElementDef alias).
/// Dispatch sites updated atomically in Phase 05.2 Wave 2 (Plan 03).
pub const ConstraintDefinition = struct {
    name: []const u8,
    modifiers: []Modifier,
    direction: Direction,
    params: ?*Node,              // optional ParameterList node
    specializes: ?*Node,         // StructuralRelationship (unchanged from ElementDef)
    body: *Node,                 // constraint_body node
    annotations: []*Node,
    doc: ?*Node,
    leading_comments: []Comment = &.{},
    doc_comment: ?*DocComment = null,
};

/// Shared shape for all usage kinds.
pub const ElementUsage = struct {
    name: []const u8,
    modifiers: []Modifier,
    direction: Direction,
    type_node: ?*Node, // type_annotation
    multiplicity: ?*Node,
    default_value: ?*Node,
    inline_body: ?*Node,
    annotations: []*Node,
    doc: ?*Node,

    // D-28 / D-29 comment attachment fields (Plan 02-01).
    leading_comments: []Comment = &.{},
    trailing_comments: []Comment = &.{},
    doc_comment: ?*DocComment = null,

    // D-06/D-08 generalized precision slot (Phase 05.2 Wave 2).
    // Points to a return_contract node — the SAME node kind used by calc returns.
    // Defaulted null so all existing usage constructors stay valid (non-breaking).
    precision: ?*Node = null,
};

pub const PartUsage = ElementUsage;
pub const PortUsage = ElementUsage;
pub const AttributeUsage = ElementUsage;
pub const ActionUsage = ElementUsage;
pub const ActorUsage = ElementUsage;
pub const SubjectUsage = ElementUsage;
pub const StateUsage = ElementUsage;
pub const ItemUsage = ElementUsage;
pub const InterfaceUsage = ElementUsage;
pub const ConnectionUsage = ElementUsage;
pub const FlowUsage = ElementUsage;
pub const AllocationUsage = ElementUsage;
pub const RequirementUsage = ElementUsage;
pub const ConstraintUsage = ElementUsage;
pub const NeedUsage = ElementUsage;
pub const UseCaseUsage = ElementUsage;

pub const TypeAnnotation = struct {
    /// Dot-separated qualified name segments
    name_segments: [][]const u8,
    /// P2 WS-C0: span of the TERMINAL name token (the last segment), for
    /// precise rename/highlight. Defaults to the empty span when unset (e.g.
    /// constructed without span tracking); callers fall back to the node span.
    terminal_span: Span = .{ .start = 0, .end = 0 },
};

pub const Multiplicity = struct {
    lower: u32,
    upper: ?u32, // null = unbounded (*)
    /// true if upper is unbounded ("*")
    unbounded: bool,
};

pub const ModifierList = struct {
    modifiers: []Modifier,
};

pub const VisibilityWrapper = struct {
    visibility: Visibility,
    members: []*Node,
};

pub const Visibility = enum { public, protected, private };

pub const Specialization = struct {
    op_text: []const u8, // e.g. "specializes"
    target_segments: [][]const u8,
};

pub const Redefinition = struct {
    op_text: []const u8,
    target_segments: [][]const u8,
    value: ?*Node,
};

pub const OperatorStatement = struct {
    op_text: []const u8,
    target_segments: [][]const u8,
    value: ?*Node,
};

pub const StructuralRelationship = struct {
    op_text: []const u8,
    target_segments: [][]const u8,
    /// P2 WS-C0: span of the TERMINAL target name token (excludes the
    /// `<<operator>>`), for precise rename/highlight. Empty span when unset.
    target_span: Span = .{ .start = 0, .end = 0 },
};

pub const InlineBody = struct {
    members: []*Node,
};

pub const Annotation = struct {
    /// Raw annotation token text (without @), e.g. "confidence"
    name: []const u8,
    /// Category prefix text if any, e.g. "simulation" in @simulation:<<computes>>
    category: ?[]const u8,
    /// Operator text if any, e.g. "computes"
    operator: ?[]const u8,
    /// Target name segments after operator
    target_segments: [][]const u8,
    /// Inline value expression (for @key: value forms)
    value: ?*Node,
    /// Annotation body { ... } if present
    body: ?*Node,
};

pub const DocComment = struct {
    text: []const u8,
};

/// A single attached comment (D-28 / D-29). Text is arena-owned; do NOT
/// free or mutate the slice (T-02-04 trust-boundary note).
pub const Comment = struct {
    /// Distinguishes `//...` from `/*...*/`. doc_comment tokens are promoted
    /// to the `doc_comment: ?*DocComment` field on declarations and never appear
    /// in leading_comments / trailing_comments arrays (D-29).
    kind: enum { line, block },
    text: []const u8,
    span: Span,
};

pub const AnnotationBodyNode = struct {
    fields: []AnnotationField,
};

pub const VerificationBlock = struct {
    fields: []VerifField,
};

pub const PreconditionBlock = struct {
    name: []const u8,
    conditions: []*Node,
};

pub const PostconditionBlock = struct {
    name: []const u8,
    conditions: []*Node,
};

pub const Binary = struct {
    op: BinaryOp,
    lhs: *Node,
    rhs: *Node,
};

pub const Unary = struct {
    op: UnaryOp,
    operand: *Node,
};

pub const MemberAccess = struct {
    receiver: *Node,
    member: []const u8,
};

pub const Call = struct {
    callee: *Node,
    args: []*Node,
};

pub const Identifier = struct {
    name: []const u8,
};

pub const IntLiteralNode = struct {
    text: []const u8,
};

pub const RealLiteralNode = struct {
    text: []const u8,
};

pub const StringLiteralNode = struct {
    value: []const u8,
};

pub const TemplateLiteralNode = struct {
    parts: []TemplatePart,
};

pub const BoolLiteralNode = struct {
    value: bool,
};

pub const InterpolationNode = struct {
    expr: *Node,
};

// ─── Composition placeholders (Plan 04 fills bodies) ──────────────────────

pub const SystemBlock = struct {
    name: ?[]const u8,
    children: []*Node,
};

pub const SubsystemBlock = struct {
    name: ?[]const u8,
    children: []*Node,
};

pub const ComponentInstance = struct {
    /// The IDENT after `[<` — e.g. "BatteryPack" in `[<BatteryPack as="battery" />]`.
    type_ref: ?[]const u8 = null,
    /// The `as="..."` value if present (the instance name).
    name: ?[]const u8 = null,
    /// Other prop=value pairs as object-field nodes.
    attrs: []*Node = &.{},
};

/// D-09: first-class comp_connect with typed slots for the four well-known
/// attributes (`from`, `to`, `via`, `carrying`). Other attributes land in
/// `other_props`. JSON emitter writes fields alphabetically:
///   carrying_expr, from_expr, other_props, to_expr, via_expr (LOCKED by D-18).
pub const CompConnect = struct {
    from_expr: ?*Node = null,
    to_expr: ?*Node = null,
    via_expr: ?*Node = null,
    carrying_expr: ?*Node = null,
    other_props: []*Node = &.{},
};

pub const ExposeTag = struct {
    /// The exposed port path — e.g. "battery.hvOut" in
    /// `[<expose battery.hvOut as="hvPowerOut" />]`. Stored as a string for
    /// Plan 04; Phase 2 may upgrade to a NamespacePath node.
    target: ?[]const u8 = null,
    /// Other prop=value pairs as object-field nodes (e.g. `as="hvPowerOut"`).
    attrs: []*Node = &.{},
};

pub const TraceabilityBlock = struct {
    children: []*Node,
};

pub const AllocateTag = struct {
    /// All `from=`/`to=`/`relationship=` etc. as object-field nodes.
    attrs: []*Node = &.{},
};

pub const SatisfyBlock = struct {
    children: []*Node,
};

/// Inline object literal field — `key: value` inside `{ ... }`.
/// Plan 04 Task 4.1b — used both by CompConnect `via=`/`carrying=` slots
/// and by Plan 04 Task 4.1c composite-block bodies.
pub const ObjectField = struct {
    key: []const u8,
    value: *Node,
    span: Span,
};

/// Inline object literal — `TypeName? { field: value, ... }`. When the
/// surrounding `{...}` is preceded by an IDENT (e.g. `CANHarness {...}`),
/// `type_name` carries that IDENT; otherwise it's null (bare `{...}`).
/// Produced by parser_dealx.parseInlineObjectOrExpr per D-08 (the inline
/// `{...}` mode-switch).
pub const ObjectLiteral = struct {
    type_name: ?[]const u8 = null,
    fields: []ObjectField = &.{},
};

/// Array literal `[item, item, ...]` (D-04). Replaces `call(__array__, items)`.
/// The single source of truth for bracketed array expressions in both
/// .deal (verification arrays) and .dealx (attribute arrays) contexts.
pub const ArrayLiteral = struct {
    items: []*Node,
};

// ─── Calc/constraint payload structs (Phase 05.2 Wave 2, SD-21/22/23) ───────

pub const CalcDefinition = struct {
    name: []const u8,
    modifiers: []Modifier,
    direction: Direction,
    params: *Node,               // ParameterList node (required)
    type_node: *Node,            // TypeAnnotation (return type, required)
    return_contract: ?*Node,     // ReturnContract node (optional)
    body: *Node,                 // calc_body node
    annotations: []*Node,
    doc: ?*Node,
    leading_comments: []Comment = &.{},
    doc_comment: ?*DocComment = null,
};

pub const ParameterList = struct {
    params: []*Node,             // slice of param_decl nodes
};

pub const ParameterDecl = struct {
    name: []const u8,
    direction: Direction,        // default .in; .out/.inout accepted (D-11)
    type_node: *Node,            // TypeAnnotation (required)
    multiplicity: ?*Node,
};

pub const ReturnContract = struct {
    items: []*Node,              // precision_spec or constraint_ref nodes
};

pub const PrecisionKind = enum { sig_figures, tolerance_absolute, tolerance_relative };

pub const PrecisionSpec = struct {
    kind: PrecisionKind,
    value: *Node,                // int_literal (sig N) or expression (± tolerance)
};

pub const ConstraintRef = struct {
    name_segments: [][]const u8, // QualifiedName (reuses TypeAnnotation pattern)
    args: ?[]*Node,              // optional arg list (same as Call.args)
};

pub const ConstraintBody = struct {
    members: []*Node,            // require_statement | local_binding | annotation nodes
};

pub const CalcBody = struct {
    bindings: []*Node,           // LocalBinding nodes
    return_stmt: *Node,          // ReturnStatement node
    annotations: []*Node,
};

pub const RequireStatement = struct {
    condition: *Node,            // ConditionExpression (reuses expression parser)
    // D-06/D-08 generalized precision slot — points to a return_contract node.
    // The require threshold/result-value precision; null when no precision contract.
    precision: ?*Node = null,
};

pub const ValidateTag = struct {
    attrs: []*Node = &.{},
};

// ─── Behavioral surface payloads (BH-1..BH-7, Stage-2 S2.2) ─────────────────
// Surface AST that mirrors deal.ebnf §9b/§9c. These are *members* of an
// ActionBody/StateBody; S2.5 lowers them (desugaring control blocks) to the
// IR v0.1 node/edge model. See spec/ir/v0.1/behavioral-mapping.md.

/// Control endpoint keyword in a flow position. `start`/`done` are stdlib
/// control actions; `terminate` → TerminateActionUsage (8.3.17.16).
pub const ControlEndpoint = enum { start, done, terminate };

/// Loop flavor. while/until → WhileLoopActionUsage (8.3.17.19);
/// for → ForLoopActionUsage (8.3.17.9).
pub const LoopKind = enum { while_loop, until_loop, for_loop };

/// State sub-action kind → StateSubactionMembership.kind (8.3.18.4).
pub const SubactionKind = enum { entry, do_action, exit };

/// A bracketed guard `[ Expression | else ]`. Helper struct (not a node):
/// `is_else` marks the `[else]` default branch; otherwise `expr` is the
/// boolean condition. Used by succession links, decide branches, transitions.
pub const Guard = struct {
    expr: ?*Node = null,
    is_else: bool = false,
};

/// One link in a succession chain: an optional guard then a flow reference.
/// The head step of a chain has `guard = null`.
pub const SuccessionStep = struct {
    guard: ?Guard = null,
    ref: *Node, // identifier / control_ref / decide_block / par_block
};

/// One branch of a decide block: a guard and its target flow reference.
pub const DecideBranch = struct {
    guard: Guard,
    target: *Node,
};

/// ActionBody / StateBody container (BH-7). Members are the behavioral nodes
/// below plus pins, sub-actions, annotations, and visibility wrappers.
pub const ActionBody = struct {
    members: []*Node,
};

/// Directed parameter (pin) of an action — `in/out/inout x : T [m]? = d?;`.
/// → directed Feature + FeatureDirectionKind (8.3.3.1.5).
pub const PinDecl = struct {
    name: []const u8,
    direction: Direction, // in | out | inout (never .none for a pin)
    type_node: *Node, // TypeAnnotation
    multiplicity: ?*Node = null,
    default_value: ?*Node = null,
};

/// `start | done | terminate` in a flow position. → SuccessionAsUsage (8.3.13.6).
pub const SuccessionChain = struct {
    steps: []SuccessionStep,
};

pub const ControlRef = struct {
    endpoint: ControlEndpoint,
};

/// `decide { [g] -> ref ... }` → DecisionNode (8.3.17.7) + implicit MergeNode.
pub const DecideBlock = struct {
    branches: []DecideBranch,
};

/// `par { -> ref ... } (-> ref)?` → ForkNode (8.3.17.8) + implicit JoinNode.
pub const ParBlock = struct {
    branches: []*Node, // each is a flow ref (the `-> ref` head of a thread)
    exit: ?*Node = null, // optional `-> ref` after the block (out of the join)
};

/// `loop while/until [g] { body }` / `for v in e { body }`.
/// → WhileLoopActionUsage (8.3.17.19) / ForLoopActionUsage (8.3.17.9).
pub const LoopStatement = struct {
    kind: LoopKind,
    guard: ?*Node = null, // while/until guard expression
    var_name: ?[]const u8 = null, // for-loop iteration variable
    iterable: ?*Node = null, // for-loop iterable expression
    body: *Node, // action_body node
};

/// `send Payload to Target?;` → SendActionUsage (8.3.17.15).
pub const SendAction = struct {
    payload_segments: [][]const u8,
    target_segments: ?[][]const u8 = null,
};

/// `accept Trigger [g]?;` → AcceptActionUsage (8.3.17.2).
pub const AcceptAction = struct {
    trigger_segments: [][]const u8,
    guard: ?*Node = null,
};

/// `assign Target := Value;` → AssignmentActionUsage (8.3.17.5).
pub const AssignAction = struct {
    target_segments: [][]const u8,
    value: *Node,
};

/// `Callee(args);` bare call → PerformActionUsage (8.3.17.14).
pub const PerformStatement = struct {
    call: *Node, // call node
};

/// `Source ~> Target (: FlowType)?;` → FlowUsage (8.3.16.3) / KerML ItemFlow.
pub const ItemFlowStatement = struct {
    source: *Node, // feature ref (identifier / member_access)
    target: *Node,
    flow_type_segments: ?[][]const u8 = null,
};

/// `bind Lhs = Rhs;` → BindingConnectorAsUsage / KerML BindingConnector (8.3.4.5.2).
pub const BindingStatement = struct {
    lhs: *Node,
    rhs: *Node,
};

/// `node Name : Type { body }?;` escape hatch (BH-1) → ActionUsage (named Step).
pub const EscapeNode = struct {
    name: []const u8,
    type_segments: [][]const u8,
    body: ?*Node = null, // optional action_body
};

/// `succession Source -> [g]? Target;` escape hatch → SuccessionAsUsage.
pub const EscapeSuccession = struct {
    source_segments: [][]const u8,
    guard: ?Guard = null,
    target_segments: [][]const u8,
};

/// `entry|do|exit / Behavior;` → StateSubactionMembership (8.3.18.4).
pub const EntryDoExit = struct {
    kind: SubactionKind,
    behavior: *Node, // call or identifier (BehaviorRef)
};

/// `on Trigger [g]? (/ Effect)? -> Target;` → TransitionUsage (8.3.18.9)
/// + TransitionFeatureMembership (8.3.18.8) for trigger/guard/effect.
pub const TransitionStatement = struct {
    trigger_segments: [][]const u8,
    guard: ?*Node = null,
    effect: ?*Node = null, // BehaviorRef (call/identifier)
    target: *Node, // target_ref node
};

/// Transition / chain target: `done | terminate | QualifiedName`.
pub const TargetRef = struct {
    endpoint: ?ControlEndpoint = null, // set for done/terminate
    name_segments: [][]const u8 = &.{}, // set for a named target
};

// ─── Payload union ────────────────────────────────────────────────────────

/// Tagged-union payload (D-01). Zig 0.16.0 enforces exhaustive switch over
/// this union — adding a NodeKind without a payload arm is a compile error.
pub const Payload = union(NodeKind) {
    // File-level
    deal_file: DealFile,
    dealx_file: DealxFile,
    header_block: HeaderBlock,
    package_decl: PackageDecl,
    import_decl: ImportDecl,
    export_decl: ExportDecl,

    // Element definitions
    part_def: PartDef,
    port_def: PortDef,
    action_def: ActionDef,
    state_def: StateDef,
    attribute_def: AttributeDef,
    item_def: ItemDef,
    interface_def: InterfaceDef,
    connection_def: ConnectionDef,
    flow_def: FlowDef,
    allocation_def: AllocationDef,
    requirement_def: RequirementDef,
    constraint_def: ConstraintDefinition,  // CHANGED from ConstraintDef alias (Phase 05.2)
    need_def: NeedDef,
    use_case_def: UseCaseDef,
    actor_def: ActorDef,

    // Calc/constraint definitions (Phase 05.2 Wave 2)
    calc_def: CalcDefinition,
    param_list: ParameterList,
    param_decl: ParameterDecl,
    return_contract: ReturnContract,
    precision_spec: PrecisionSpec,
    constraint_ref: ConstraintRef,
    constraint_body: ConstraintBody,
    calc_body: CalcBody,
    require_statement: RequireStatement,

    // Behavioral surface (BH-1..BH-7, Stage-2 S2.2)
    action_body: ActionBody,
    pin_decl: PinDecl,
    succession_chain: SuccessionChain,
    control_ref: ControlRef,
    decide_block: DecideBlock,
    par_block: ParBlock,
    loop_statement: LoopStatement,
    send_action: SendAction,
    accept_action: AcceptAction,
    assign_action: AssignAction,
    perform_statement: PerformStatement,
    item_flow_statement: ItemFlowStatement,
    binding_statement: BindingStatement,
    escape_node: EscapeNode,
    escape_succession: EscapeSuccession,
    entry_do_exit: EntryDoExit,
    transition_statement: TransitionStatement,
    target_ref: TargetRef,

    // Usages
    part_usage: PartUsage,
    port_usage: PortUsage,
    attribute_usage: AttributeUsage,
    action_usage: ActionUsage,
    actor_usage: ActorUsage,
    subject_usage: SubjectUsage,
    state_usage: StateUsage,
    item_usage: ItemUsage,
    interface_usage: InterfaceUsage,
    connection_usage: ConnectionUsage,
    flow_usage: FlowUsage,
    allocation_usage: AllocationUsage,
    requirement_usage: RequirementUsage,
    constraint_usage: ConstraintUsage,
    need_usage: NeedUsage,
    use_case_usage: UseCaseUsage,

    // Type/structural
    type_annotation: TypeAnnotation,
    multiplicity: Multiplicity,
    modifier_list: ModifierList,
    visibility_wrapper: VisibilityWrapper,
    specialization: Specialization,
    redefinition: Redefinition,
    operator_statement: OperatorStatement,
    inline_body: InlineBody,
    structural_relationship: StructuralRelationship,

    // Annotations
    annotation: Annotation,
    doc_comment: DocComment,
    annotation_body: AnnotationBodyNode,

    // Verification/precondition/postcondition
    verification_block: VerificationBlock,
    precondition_block: PreconditionBlock,
    postcondition_block: PostconditionBlock,

    // Expressions
    binary: Binary,
    unary: Unary,
    member_access: MemberAccess,
    call: Call,
    identifier: Identifier,
    int_literal: IntLiteralNode,
    real_literal: RealLiteralNode,
    string_literal: StringLiteralNode,
    template_literal: TemplateLiteralNode,
    boolean_literal: BoolLiteralNode,
    interpolation: InterpolationNode,
    array_literal: ArrayLiteral,

    // Composition placeholders
    system_block: SystemBlock,
    subsystem_block: SubsystemBlock,
    component_instance: ComponentInstance,
    comp_connect: CompConnect,
    expose_tag: ExposeTag,
    traceability_block: TraceabilityBlock,
    allocate_tag: AllocateTag,
    satisfy_block: SatisfyBlock,
    validate_tag: ValidateTag,
    object_literal: ObjectLiteral,
};

// ─── Node ──────────────────────────────────────────────────────────────────

/// AST node (D-01, D-15). Every node carries kind + span + payload.
/// Allocated via the per-handle arena (D-02) — never stored outside.
pub const Node = struct {
    kind: NodeKind,
    span: Span,
    payload: Payload,
};
