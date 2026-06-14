//! DEAL C ABI surface (D-10, D-11, D-12, D-13).
//!
//! Ten exports (after Phase 5 Plan 01), all `callconv(.c)` lowercase:
//!   deal_parse, deal_free, deal_has_errors,
//!   deal_diagnostics_count, deal_ast_json, deal_diagnostics_json,
//!   deal_ir_json, deal_format, deal_index_json,
//!   deal_check_with_stdlib.
//! deal_format (D-21) is the 8th C ABI export, added in Plan 02-05.
//! deal_index_json (D-48) is the 9th, added in Phase 2 closeout.
//! deal_check_with_stdlib (D-85, D-88) is the 10th, added in Phase 5 Plan 01.
//!
//! Memory model (D-02, D-11, D-13):
//!   - Each handle owns its own ArenaAllocator backed by page_allocator.
//!   - Source bytes + filename are duped into the arena on parse.
//!   - JSON outputs are cached lazily inside the arena on first read.
//!   - `deal_free` runs `arena.deinit()` — single-shot release of every-
//!     thing the handle ever allocated. No global state, no shared mutex.
//!
//! Security controls (RESEARCH §Security Domain, Plan 06 task 6.1):
//!   - V5 ASVS: `std.unicode.utf8ValidateSlice` guards entry into the lexer/
//!     parser. Invalid UTF-8 → E0001 diagnostic, handle returned with no AST.
//!   - V12 ASVS: source_len > std.math.maxInt(u32) → E0004 diagnostic; guard
//!     fires BEFORE any buffer access so there is no integer-overflow risk on
//!     u32 span fields (D-15).

const std = @import("std");
const ast = @import("ast");
const lexer = @import("lexer");
const parser = @import("parser");
const diagnostics = @import("diagnostics");
const json = @import("json");
const sema = @import("sema");
const ir = @import("ir");
const lowering = @import("lowering");
const fmt = @import("fmt");

/// Per-parse arena-rooted state. Allocated inside its own arena so a single
/// `arena.deinit()` releases every byte the handle owns (D-02).
///
/// Zig 0.16.0 `ArrayList(T) = .empty` allocator-explicit shape: the list
/// holds no allocator reference; callers pass the allocator on each append /
/// deinit. The arena lives on the handle, so calls look like
/// `h.diagnostics.append(h.arena.allocator(), diag)`.
pub const DealHandle = struct {
    arena: std.heap.ArenaAllocator,
    ast_root: ?*ast.Node = null,
    /// Symbol table produced by the semantic analyzer (D-20).
    /// Plan 02-03 uses this for IR lowering.
    index_root: ?*sema.SymbolTable = null,
    mode: ast.Mode = .deal,
    source: []const u8 = "",
    filename: []const u8 = "",
    diagnostics: std.ArrayList(diagnostics.Diagnostic) = .empty,
    cached_ast_json: ?[]const u8 = null,
    cached_diag_json: ?[]const u8 = null,
    /// IR document produced by the lowering pass (Plan 02-03, D-22..D-27).
    /// Populated lazily by deal_ir_json() on first call.
    ir_root: ?*ir.Document = null,
    /// Cached IR JSON (arena-owned). Populated lazily by deal_ir_json().
    cached_ir_json: ?[]const u8 = null,
    /// Cached formatted source bytes (arena-owned). Populated lazily by deal_format().
    cached_fmt_bytes: ?[]const u8 = null,
    /// Cached index JSON (arena-owned). Populated lazily by deal_index_json().
    cached_index_json: ?[]const u8 = null,
};

/// Convert an opaque ABI pointer back to the typed handle. Caller has
/// already null-checked.
inline fn handleCast(p: *anyopaque) *DealHandle {
    return @ptrCast(@alignCast(p));
}

/// Parse a source buffer. Mode is selected from the filename extension
/// (D-12); `.dealx` suffix → `.dealx`, otherwise `.deal`. Returns a non-null
/// handle on success (D-10); allocator failure (or NULL ptr paired with a
/// non-zero length on either buffer, see CR-01) produces a null result.
///
/// Step ordering (WR-07):
///   Step 0: NULL-pointer guards on the two input buffers (CR-01).
///   Step 1: allocate arena + handle (OOM → null result).
///   Step 2: dupe filename into the arena (OOM → arena.deinit() + null).
///   Step 3: V12 bound check on source_len (> maxInt(u32) → E0004, return
///           the partially-initialized handle with empty source).
///   Step 4: dupe source bytes into the arena (OOM → arena.deinit() + null).
///   Step 5: V5 UTF-8 validation on the duped source (invalid → E0001,
///           return handle with no AST).
///   Step 6: invoke parser.parseFile.
///
/// Steps 1-2 can fail before the V12 check fires; that ordering is
/// intentional because the V12 emission path needs a handle to attach the
/// diagnostic to. Steps 3 and 5 are the only security controls in this
/// function — both fire before the parser is invoked.
pub export fn deal_parse(
    source_ptr: [*]const u8,
    source_len: usize,
    filename_ptr: [*]const u8,
    filename_len: usize,
) callconv(.c) ?*anyopaque {
    // Step 0: guard against NULL pointers paired with non-zero length (caller
    // misuse, ASVS V5 input validation). Zero-length is OK regardless of the
    // pointer value because we never dereference. This mirrors the existing
    // NULL guard in `deal_free` so input validation is symmetric across all
    // six C exports. The 36 documented threats did not cover "caller passes
    // NULL with non-zero length"; CR-01 closes that gap.
    if (source_len > 0 and @intFromPtr(source_ptr) == 0) return null;
    if (filename_len > 0 and @intFromPtr(filename_ptr) == 0) return null;

    // Step 1: allocate arena + handle (D-10 — null only on alloc failure).
    // Use a temporary init_alloc only for the handle creation; then switch to
    // handle.arena.allocator() for ALL subsequent allocations (arena aliasing
    // fix — see deal_parse_internal design note).
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    const init_alloc = arena.allocator();
    const handle = init_alloc.create(DealHandle) catch {
        arena.deinit();
        return null;
    };
    handle.* = .{ .arena = arena, .source = "", .filename = "" };

    // From here, use handle.arena.allocator() for all allocations so they
    // are tracked by the arena stored IN the handle, not the stale local copy.
    const alloc = handle.arena.allocator();

    // Step 2: dupe filename + decide mode (D-12 — extension-based).
    const filename_slice = filename_ptr[0..filename_len];
    handle.filename = alloc.dupe(u8, filename_slice) catch {
        handle.arena.deinit();
        return null;
    };
    handle.mode = if (std.mem.endsWith(u8, handle.filename, ".dealx")) .dealx else .deal;

    // Step 3: V12 bound check — source_len must fit in u32 (D-15 span is u32).
    // This guard fires BEFORE any buffer read so there is no integer-overflow
    // risk on span arithmetic downstream.
    if (source_len > std.math.maxInt(u32)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_source_too_large,
            .severity = .err,
            .message = "source larger than 4 GiB; spans cannot address it",
            .span = .{ .start = 0, .end = 0 },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        return @ptrCast(handle);
    }

    // Step 4: dupe source bytes (D-13 — independent of caller buffer).
    const source_slice = source_ptr[0..source_len];
    handle.source = alloc.dupe(u8, source_slice) catch {
        handle.arena.deinit();
        return null;
    };

    // Step 5: V5 UTF-8 validation (RESEARCH line 991, line 519, line 1010).
    // Invalid UTF-8 can crash the lexer's byte walker if it tries to interpret
    // malformed multi-byte sequences. Reject the entire input here; the caller
    // can read diagnostics via deal_diagnostics_json (D-10 — non-null handle).
    if (!std.unicode.utf8ValidateSlice(handle.source)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_invalid_utf8,
            .severity = .err,
            .message = "source bytes are not valid UTF-8",
            .span = .{ .start = 0, .end = @intCast(@min(handle.source.len, std.math.maxInt(u32))) },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        // D-10 — return handle with no ast_root; do NOT attempt parsing.
        return @ptrCast(handle);
    }

    // Step 6: parse (Plan 03/04 work). On OOM the parser returns an error;
    // we keep ast_root null so callers see an empty AST (D-10: handle is
    // always non-null, ast_root may be null on parse failure).
    handle.ast_root = parser.parseFile(
        handle.arena.allocator(),
        handle.source,
        handle.mode,
        &handle.diagnostics,
    ) catch null;

    // Step 7: semantic analysis (Plan 02-02, D-20). Runs after parse; sema
    // diagnostics flow into the same handle.diagnostics list. On OOM we keep
    // index_root null; diagnostics already collected are preserved.
    handle.index_root = sema.analyze(
        handle.arena.allocator(),
        handle.ast_root,
        &handle.diagnostics,
        handle.filename,
    ) catch null;

    return @ptrCast(handle);
}

/// Test-only entry point — NOT exported to the C ABI (no `callconv(.c)`,
/// no `export` keyword). Accepts a caller-provided allocator so unit tests
/// can back the arena with `std.testing.allocator` for leak detection.
///
/// Returns the typed `*DealHandle` directly (no opaque cast) because the
/// test code needs to reach `.arena` for an explicit deinit.
///
/// MUST NOT appear in `nm -gU zig-out/lib/libdeal.a` — Zig only exports
/// `pub export fn`; a bare `pub fn` is reachable from in-tree tests but
/// NOT emitted as a public dynamic symbol (Plan 06 Task 6.1 acceptance
/// criterion). Verified by `nm ... | grep _internal | wc -l` → 0.
pub fn deal_parse_internal(
    backing_allocator: std.mem.Allocator,
    source: []const u8,
    filename: []const u8,
) !*DealHandle {
    // Defensive NULL-pointer guard symmetric with `deal_parse` (CR-01). Zig
    // slices carry their own length, but a caller could synthesize a slice
    // with a NULL underlying pointer and non-zero length via @ptrCast tricks.
    // Treat that as bad input the same way the C ABI entry does.
    if (source.len > 0 and @intFromPtr(source.ptr) == 0) return error.InvalidArgument;
    if (filename.len > 0 and @intFromPtr(filename.ptr) == 0) return error.InvalidArgument;

    // IMPORTANT: create the arena with backing_allocator, allocate the handle
    // inside it, copy the arena into the handle, then use ONLY
    // handle.arena.allocator() for all subsequent allocations. Using the local
    // `allocator` obtained before the copy leads to aliasing between the local
    // arena struct and handle.arena — subsequent allocations tracked by one
    // may not be freed when the other is deinit'd. This pattern mirrors
    // deal_parse (production code) exactly.
    var arena = std.heap.ArenaAllocator.init(backing_allocator);

    // Use a temporary allocator ONLY for the initial handle creation.
    const init_alloc = arena.allocator();
    const handle = try init_alloc.create(DealHandle);
    handle.* = .{ .arena = arena, .source = "", .filename = "" };

    // From this point on, ALL allocations use handle.arena.allocator() to
    // ensure they are tracked by the arena stored in the handle (not the stale
    // local copy). This is critical for leak-free operation with
    // std.testing.allocator as the backing allocator.
    const alloc = handle.arena.allocator();

    handle.filename = try alloc.dupe(u8, filename);
    handle.mode = if (std.mem.endsWith(u8, handle.filename, ".dealx")) .dealx else .deal;

    // V12 bound check (same as deal_parse).
    if (source.len > std.math.maxInt(u32)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_source_too_large,
            .severity = .err,
            .message = "source larger than 4 GiB; spans cannot address it",
            .span = .{ .start = 0, .end = 0 },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        return handle;
    }

    handle.source = try alloc.dupe(u8, source);

    // V5 UTF-8 validation (same as deal_parse).
    if (!std.unicode.utf8ValidateSlice(handle.source)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_invalid_utf8,
            .severity = .err,
            .message = "source bytes are not valid UTF-8",
            .span = .{ .start = 0, .end = @intCast(@min(handle.source.len, std.math.maxInt(u32))) },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        return handle;
    }

    handle.ast_root = parser.parseFile(
        handle.arena.allocator(),
        handle.source,
        handle.mode,
        &handle.diagnostics,
    ) catch null;

    // Semantic analysis (mirrors deal_parse Step 7).
    handle.index_root = sema.analyze(
        handle.arena.allocator(),
        handle.ast_root,
        &handle.diagnostics,
        handle.filename,
    ) catch null;

    return handle;
}

/// Test-only helper: write `.deal/index.json` to `<out_dir>/.deal/index.json`.
/// Uses the handle's symbol table. NOT exported to the C ABI (no `callconv(.c)`).
/// Used by sema_corpus.zig to exercise the on-disk alphabetical-key contract (D-18).
pub fn write_index_json_to_path(handle: *DealHandle, out_dir: []const u8) !void {
    const table = handle.index_root orelse return error.NoSymbolTable;
    const json_bytes = try json.writeIndexJson(
        handle.arena.allocator(),
        table,
        "0.1.0-phase2", // deal version for Phase 2
    );

    // Create the .deal/ subdirectory under out_dir.
    const dir_path = try std.fmt.allocPrint(handle.arena.allocator(), "{s}/.deal", .{out_dir});
    // Use std.fs.cwd() to create the directory.
    std.fs.cwd().makePath(dir_path) catch |err| {
        if (err != error.PathAlreadyExists) return err;
    };

    // Write the file.
    const file_path = try std.fmt.allocPrint(handle.arena.allocator(), "{s}/.deal/index.json", .{out_dir});
    const file = try std.fs.cwd().createFile(file_path, .{ .truncate = true });
    defer file.close();
    try file.writeAll(json_bytes);
}

/// Test-only counterpart to `deal_parse_internal`. Deinits the handle's arena
/// (which also frees the handle itself since it was allocated inside the
/// arena). The caller must not access `handle` after this call.
///
/// NOT exported to the C ABI (no `callconv(.c)`, no `export`).
pub fn deal_free_internal(handle: *DealHandle) void {
    var arena = handle.arena;
    arena.deinit();
}

/// Source file descriptor for `deal_parse_internal_with_stdlib`.
pub const StdlibSource = struct {
    /// DEAL source text (UTF-8).
    source: []const u8,
    /// Filename used in diagnostics (e.g. "units/si.deal").
    filename: []const u8,
};

/// Parse ONE stdlib source file, run sema Pass A/B, and merge its
/// dimension_def / unit_def entries into `merged` (first declaration wins).
///
/// Shared by `deal_parse_internal_with_stdlib` (slice of sources) and the
/// C ABI `deal_check_with_stdlib` (NUL-delimited buffer split into sources).
/// A single `parseFile` over multiple concatenated @header/package files
/// fails at the second file boundary, so each stdlib file MUST be parsed
/// independently — this helper is that per-file unit.
///
/// Unparseable or non-UTF-8 sources are skipped silently (stdlib seeding is
/// best-effort; a bad stdlib file must never abort the user's check).
/// `alloc` must own `merged` and outlive `analyzeWithExternalTable`.
fn mergeStdlibSourceInto(
    merged: *sema.SymbolTable,
    alloc: std.mem.Allocator,
    source: []const u8,
    filename: []const u8,
) void {
    if (source.len == 0) return;
    if (!std.unicode.utf8ValidateSlice(source)) return;

    var parse_diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    const mode: ast.Mode = if (std.mem.endsWith(u8, filename, ".dealx")) .dealx else .deal;
    const root = parser.parseFile(alloc, source, mode, &parse_diags) catch null;
    if (root == null) return;

    var sub_diags: std.ArrayList(diagnostics.Diagnostic) = .empty;
    const sub_table = sema.analyze(alloc, root, &sub_diags, filename) catch return;

    var it = sub_table.entries.iterator();
    while (it.next()) |kv| {
        const entry = kv.value_ptr.*;
        if (entry.kind == .dimension_def or entry.kind == .unit_def) {
            if (!merged.entries.contains(kv.key_ptr.*)) {
                const key_copy = alloc.dupe(u8, kv.key_ptr.*) catch continue;
                merged.entries.put(key_copy, entry) catch continue;
            }
        }
    }
}

/// Test-only variant of `deal_parse_internal` that seeds the sema with
/// dimension/unit metadata from stdlib sources BEFORE analyzing the target file.
///
/// Used by the dimensional regression test harness and showcase-clean loop.
/// Caller passes stdlib source slices; this function:
///  1. Parses each stdlib file and runs sema Pass A (declaration collection).
///  2. Merges all dimension_def / unit_def entries into a single external table.
///  3. Runs `sema.analyzeWithExternalTable` on the target `source`/`filename`.
///
/// The returned handle owns its own arena (independent of stdlib_sources).
/// Stdlib sources are only alive during the seeding phase; the external table's
/// entries are cloned into the handle's arena.
///
/// NOT exported to the C ABI.
pub fn deal_parse_internal_with_stdlib(
    backing_allocator: std.mem.Allocator,
    source: []const u8,
    filename: []const u8,
    stdlib_sources: []const StdlibSource,
) !*DealHandle {
    if (source.len > 0 and @intFromPtr(source.ptr) == 0) return error.InvalidArgument;
    if (filename.len > 0 and @intFromPtr(filename.ptr) == 0) return error.InvalidArgument;

    // ── Phase 1: Build external (stdlib) symbol table ─────────────────────
    //
    // Use a separate arena for stdlib parsing so we can discard it cleanly
    // after merging entries into the handle arena.
    var stdlib_arena = std.heap.ArenaAllocator.init(backing_allocator);
    defer stdlib_arena.deinit();
    const stdlib_alloc = stdlib_arena.allocator();

    const merged = try stdlib_alloc.create(sema.SymbolTable);
    merged.* = .{
        .entries = std.StringHashMap(*sema.SymbolEntry).init(stdlib_alloc),
        .imported_packages = std.StringHashMap(void).init(stdlib_alloc),
        .imports_graph = &.{},
        .package_segments = &.{},
    };

    for (stdlib_sources) |src| {
        mergeStdlibSourceInto(merged, stdlib_alloc, src.source, src.filename);
    }

    // ── Phase 2: Analyze the target file with stdlib seeding ──────────────
    var arena = std.heap.ArenaAllocator.init(backing_allocator);

    const init_alloc = arena.allocator();
    const handle = try init_alloc.create(DealHandle);
    handle.* = .{ .arena = arena, .source = "", .filename = "" };

    const alloc = handle.arena.allocator();

    handle.filename = try alloc.dupe(u8, filename);
    handle.mode = if (std.mem.endsWith(u8, handle.filename, ".dealx")) .dealx else .deal;

    if (source.len > std.math.maxInt(u32)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_source_too_large,
            .severity = .err,
            .message = "source larger than 4 GiB; spans cannot address it",
            .span = .{ .start = 0, .end = 0 },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        return handle;
    }

    handle.source = try alloc.dupe(u8, source);

    if (!std.unicode.utf8ValidateSlice(handle.source)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_invalid_utf8,
            .severity = .err,
            .message = "source bytes are not valid UTF-8",
            .span = .{ .start = 0, .end = @intCast(@min(handle.source.len, std.math.maxInt(u32))) },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        return handle;
    }

    handle.ast_root = parser.parseFile(
        handle.arena.allocator(),
        handle.source,
        handle.mode,
        &handle.diagnostics,
    ) catch null;

    // Use analyzeWithExternalTable so stdlib-declared units/dimensions are visible.
    handle.index_root = sema.analyzeWithExternalTable(
        handle.arena.allocator(),
        handle.ast_root,
        &handle.diagnostics,
        handle.filename,
        merged, // stdlib seed
    ) catch null;

    return handle;
}

/// Check DEAL source bytes with a stdlib seed and emit diagnostics JSON.
///
/// C ABI export #10 — deal_check_with_stdlib (Phase 5 / D-85, D-88).
///
/// This is the blocking prerequisite for the Phase 5 verify engine (D-85) and
/// the E2500 carryover CLI integration (D-88). It exposes `analyzeWithExternalTable`
/// across the C ABI boundary so the Rust orchestrator can check a DEAL file with
/// the stdlib dimension/unit table seeded, without having to re-parse stdlib source
/// from the Rust side.
///
/// Parameters:
///   source_ptr/source_len   — DEAL source bytes to analyze (UTF-8).
///   filename_ptr/filename_len — Filename used for diagnostics + mode selection.
///   stdlib_ir_ptr/stdlib_ir_len — Stdlib DEAL source bytes (pre-built stdlib
///       content in DEAL source format). Parsed + analyzed to build the external
///       dimension/unit symbol table before analyzing the target source.
///       Pass zero-length to analyze without stdlib seeding.
///   out_diag_ptr/out_diag_len — Output: written with pointer+length to arena-
///       owned diagnostics JSON (same format as deal_diagnostics_json). The
///       buffer is owned by the returned handle's arena; clone before deal_free.
///
/// Returns:
///   Non-null opaque handle on success. Returns NULL on allocator failure or
///   NULL pointer paired with non-zero length. Caller MUST call deal_free() on
///   the returned handle to release the arena (same as deal_parse).
///
/// Diagnostics (has_errors) are available via deal_has_errors(handle) as well
/// as via out_diag_ptr/out_diag_len (pre-serialized JSON for convenience).
///
/// SECURITY (T-05-01, T-05-02, ASVS V5):
///   - All NULL+non-zero length pairs are rejected at entry (mirror deal_parse).
///   - stdlib_ir parse failure is silently skipped — analysis proceeds with an
///     empty external table (T-05-02: no panic across C ABI on bad JSON/source).
///   - Source > 4 GiB emits E0004; handle returned with diagnostics (no parse).
///   - Invalid UTF-8 emits E0001; handle returned with diagnostics (no parse).
///
/// Ownership: the caller must call `deal_free(handle)` on the returned pointer
/// to release the arena. The out_diag_ptr/out_diag_len buffer becomes invalid
/// after `deal_free`. Clone the bytes before calling deal_free (Pitfall 3).
pub export fn deal_check_with_stdlib(
    source_ptr: [*]const u8,
    source_len: usize,
    filename_ptr: [*]const u8,
    filename_len: usize,
    stdlib_ir_ptr: [*]const u8,
    stdlib_ir_len: usize,
    out_diag_ptr: *[*]const u8,
    out_diag_len: *usize,
) callconv(.c) ?*anyopaque {
    // Step 0: Guard NULL + non-zero length pairs (ASVS V5 / T-05-01).
    if (source_len > 0 and @intFromPtr(source_ptr) == 0) return null;
    if (filename_len > 0 and @intFromPtr(filename_ptr) == 0) return null;
    if (stdlib_ir_len > 0 and @intFromPtr(stdlib_ir_ptr) == 0) return null;

    // Step 1: allocate arena + handle (mirrors deal_parse lifecycle).
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    const init_alloc = arena.allocator();
    const handle = init_alloc.create(DealHandle) catch {
        arena.deinit();
        return null;
    };
    handle.* = .{ .arena = arena, .source = "", .filename = "" };
    const alloc = handle.arena.allocator();

    // Step 2: dupe filename + select parse mode.
    handle.filename = alloc.dupe(u8, filename_ptr[0..filename_len]) catch {
        handle.arena.deinit();
        return null;
    };
    handle.mode = if (std.mem.endsWith(u8, handle.filename, ".dealx")) .dealx else .deal;

    // Step 3: V12 bound check — source_len must fit in u32 (D-15).
    if (source_len > std.math.maxInt(u32)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_source_too_large,
            .severity = .err,
            .message = "source larger than 4 GiB; spans cannot address it",
            .span = .{ .start = 0, .end = 0 },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        const buf = json.emitDiagnostics(alloc, handle.diagnostics.items) catch {
            out_diag_ptr.* = "[]".ptr;
            out_diag_len.* = 2;
            return @ptrCast(handle);
        };
        out_diag_ptr.* = buf.ptr;
        out_diag_len.* = buf.len;
        return @ptrCast(handle);
    }

    // Step 4: dupe source bytes.
    handle.source = alloc.dupe(u8, source_ptr[0..source_len]) catch {
        handle.arena.deinit();
        return null;
    };

    // Step 5: V5 UTF-8 validation.
    if (!std.unicode.utf8ValidateSlice(handle.source)) {
        const diag = diagnostics.Diagnostic{
            .code = diagnostics.Codes.e_invalid_utf8,
            .severity = .err,
            .message = "source bytes are not valid UTF-8",
            .span = .{ .start = 0, .end = @intCast(@min(handle.source.len, std.math.maxInt(u32))) },
        };
        handle.diagnostics.append(alloc, diag) catch {};
        const buf = json.emitDiagnostics(alloc, handle.diagnostics.items) catch {
            out_diag_ptr.* = "[]".ptr;
            out_diag_len.* = 2;
            return @ptrCast(handle);
        };
        out_diag_ptr.* = buf.ptr;
        out_diag_len.* = buf.len;
        return @ptrCast(handle);
    }

    // Step 6: Build external (stdlib) symbol table from stdlib source bytes
    // (T-05-02: parse failures are skipped silently — analysis proceeds with
    // empty external table, no panic across C ABI boundary).
    //
    // Use a dedicated stdlib arena so stdlib allocations coexist safely with
    // the handle arena during the merge phase. The stdlib arena is deferred
    // (freed) after the merge is complete — entries are copied by key into
    // the stdlib arena; the entry pointers live in the stdlib arena and are
    // referenced by the merged table only during analyzeWithExternalTable.
    // analyzeWithExternalTable copies keys into its own arena, so the stdlib
    // arena's lifetime only needs to cover the analyzeWithExternalTable call.
    var stdlib_arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer stdlib_arena.deinit();
    const stdlib_alloc = stdlib_arena.allocator();

    const external_table = stdlib_alloc.create(sema.SymbolTable) catch null;
    if (external_table) |merged| {
        merged.* = .{
            .entries = std.StringHashMap(*sema.SymbolEntry).init(stdlib_alloc),
            .imported_packages = std.StringHashMap(void).init(stdlib_alloc),
            .imports_graph = &.{},
            .package_segments = &.{},
        };

        // Parse + analyze the stdlib source bytes to collect dimension/unit entries.
        //
        // The CLI passes one or more stdlib files concatenated with a NUL (0x00)
        // separator. NUL never occurs in valid UTF-8 DEAL source, so it is a safe
        // record delimiter. Each file is parsed INDEPENDENTLY — a single parseFile
        // over multiple @header/package files would fail at the second file's
        // @header boundary, silently yielding an empty stdlib table (the bug this
        // replaces). A buffer with no NUL is treated as a single file, preserving
        // the prior single-file behavior.
        if (stdlib_ir_len > 0) {
            const stdlib_blob = stdlib_ir_ptr[0..stdlib_ir_len];
            var file_it = std.mem.splitScalar(u8, stdlib_blob, 0);
            var file_idx: usize = 0;
            while (file_it.next()) |chunk| {
                if (chunk.len == 0) continue;
                const fname = std.fmt.allocPrint(
                    stdlib_alloc,
                    "stdlib_{d}.deal",
                    .{file_idx},
                ) catch "stdlib.deal";
                mergeStdlibSourceInto(merged, stdlib_alloc, chunk, fname);
                file_idx += 1;
            }
        }
    }

    // Step 7: Parse the target source.
    handle.ast_root = parser.parseFile(
        handle.arena.allocator(),
        handle.source,
        handle.mode,
        &handle.diagnostics,
    ) catch null;

    // Step 8: Analyze with external stdlib table.
    // analyzeWithExternalTable copies all needed keys/metadata into the handle's
    // arena, so it is safe to deinit stdlib_arena after this call.
    handle.index_root = sema.analyzeWithExternalTable(
        handle.arena.allocator(),
        handle.ast_root,
        &handle.diagnostics,
        handle.filename,
        external_table,
    ) catch null;

    // Step 9: Serialize diagnostics to JSON and write output pointers.
    // Cache in handle so deal_diagnostics_json() also works on this handle.
    const buf = json.emitDiagnostics(alloc, handle.diagnostics.items) catch {
        out_diag_ptr.* = "[]".ptr;
        out_diag_len.* = 2;
        return @ptrCast(handle);
    };
    handle.cached_diag_json = buf;
    out_diag_ptr.* = buf.ptr;
    out_diag_len.* = buf.len;

    return @ptrCast(handle);
}

/// Release every byte the handle ever allocated. Safe to call on null.
///
/// Copy the arena out before deinit per RESEARCH §Pattern 6: the handle
/// itself is allocated inside the arena, so `arena.deinit()` invalidates
/// the handle pointer. Take the arena by value first.
pub export fn deal_free(handle_ptr: ?*anyopaque) callconv(.c) void {
    const p = handle_ptr orelse return;
    const h = handleCast(p);
    var arena = h.arena;
    arena.deinit();
}

/// Returns true if the handle carries one or more diagnostics.
/// Returns false on a NULL handle (T-06-04 null-guard).
pub export fn deal_has_errors(handle_ptr: ?*anyopaque) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    return h.diagnostics.items.len > 0;
}

/// Total number of diagnostics on the handle (all severities).
/// Returns 0 on a NULL handle (T-06-04 null-guard).
pub export fn deal_diagnostics_count(handle_ptr: ?*anyopaque) callconv(.c) u32 {
    const p = handle_ptr orelse return 0;
    const h = handleCast(p);
    return @intCast(h.diagnostics.items.len);
}

/// Lazy AST-JSON emission (D-11). First call allocates inside the arena and
/// caches on the handle; subsequent calls return the cached buffer. The
/// buffer is freed by `deal_free`. Returns false on NULL handle or OOM.
pub export fn deal_ast_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_ast_json == null) {
        const buf = json.emitAst(h.arena.allocator(), h.ast_root, h.mode, h.filename) catch {
            return false;
        };
        h.cached_ast_json = buf;
    }
    const cached = h.cached_ast_json.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}

/// Lazy diagnostics-JSON emission (D-11). Same caching contract as
/// `deal_ast_json`. Returns false on NULL handle or OOM.
pub export fn deal_diagnostics_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_diag_json == null) {
        const buf = json.emitDiagnostics(h.arena.allocator(), h.diagnostics.items) catch {
            return false;
        };
        h.cached_diag_json = buf;
    }
    const cached = h.cached_diag_json.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}

/// Lazy IR-JSON emission (D-22). Runs the lowering pass on first call,
/// serializes the IR Document to JSON (spec/ir/v0/schema.json), and caches
/// the result in the handle's arena. Subsequent calls return the same
/// (ptr, len) pair — the buffer is immutable after first generation.
///
/// Returns false on NULL handle, OOM, or if the handle has no AST root
/// (e.g. parse failed due to invalid UTF-8 or source too large). The IR
/// is produced even when sema diagnostics exist (partial models are useful
/// for downstream consumers that tolerate unknown references).
///
/// EXPORT #7 — deal_ir_json is the 7th C ABI export. Plan 02-04 adds
/// deal_format as the 8th export.
pub export fn deal_ir_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_ir_json == null) {
        const alloc = h.arena.allocator();
        // Run lowering if not yet done. Lowering requires a parsed AST and
        // symbol table; if either is null (e.g. parse failed), return false.
        if (h.ir_root == null) {
            const ast_root = h.ast_root orelse return false;
            const sym_table = h.index_root orelse return false;
            h.ir_root = lowering.lower(
                alloc,
                ast_root,
                sym_table,
                h.filename,
            ) catch return false;
        }
        const doc = h.ir_root orelse return false;
        const buf = json.emitIrJson(alloc, doc) catch return false;
        h.cached_ir_json = buf;
    }
    const cached = h.cached_ir_json.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}

/// Test-only entry point for the IR lowering pass — NOT exported to the
/// C ABI (no `callconv(.c)`, no `export` keyword). Accepts a caller-
/// provided allocator so unit tests can back the arena with
/// `std.testing.allocator` for leak detection.
///
/// Returns a fully-lowered `*ir.Document` (arena-owned by the caller-provided
/// arena). The caller must deinit the arena to release all allocations.
///
/// Usage pattern (no leaks):
///   var arena = std.heap.ArenaAllocator.init(backing);
///   defer arena.deinit();
///   const doc = try deal_lower_internal(arena.allocator(), source, filename);
///
/// MUST NOT appear in `nm -gU zig-out/lib/libdeal.a`. Verified by the
/// Plan 02-03 nm gate: `nm ... | grep deal_lower_internal | wc -l` → 0.
pub fn deal_lower_internal(
    alloc: std.mem.Allocator,
    source: []const u8,
    filename: []const u8,
) !*ir.Document {
    const mode: ast.Mode = if (std.mem.endsWith(u8, filename, ".dealx")) .dealx else .deal;

    var diag_list: std.ArrayList(diagnostics.Diagnostic) = .empty;
    const ast_root_opt = try parser.parseFile(alloc, source, mode, &diag_list);
    const ast_root = ast_root_opt orelse return error.ParseFailed;

    const sym_table = try sema.analyze(alloc, ast_root, &diag_list, filename);

    return lowering.lower(alloc, ast_root, sym_table, filename);
}

/// Lazy formatted-source emission (D-21). Runs the pretty-printer on the
/// AST on first call, caches the result in the handle's arena. Subsequent
/// calls return the same (ptr, len) pair — the buffer is immutable after
/// first generation.
///
/// Returns false on NULL handle, OOM, or if the handle has no AST root
/// (e.g. parse failed due to invalid UTF-8 or source too large).
///
/// EXPORT #8 — deal_format is the 8th C ABI export. Added in Plan 02-05
/// (D-21). The formatted output is canonical DEAL source bytes (NOT JSON).
/// Caller must clone bytes BEFORE calling deal_free (Pitfall 3).
pub export fn deal_format(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_fmt_bytes == null) {
        const alloc = h.arena.allocator();
        // Formatter walks AST — no IR needed (D-25).
        const buf = fmt.emitFormatted(alloc, h.ast_root) catch return false;
        h.cached_fmt_bytes = buf;
    }
    const cached = h.cached_fmt_bytes.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}

/// Test-only entry point for the formatter — NOT exported to the C ABI
/// (no `callconv(.c)`, no `export` keyword). Accepts a caller-provided
/// allocator so unit tests can back the arena with `std.testing.allocator`
/// for leak detection.
///
/// Returns the formatted source bytes as an arena-owned slice.
/// Caller must free via the provided allocator when done.
///
/// MUST NOT appear in `nm -gU zig-out/lib/libdeal.a` (S-7 invariant).
pub fn deal_format_internal(
    backing_allocator: std.mem.Allocator,
    source: []const u8,
    filename: []const u8,
) ![]const u8 {
    const handle = try deal_parse_internal(backing_allocator, source, filename);
    defer deal_free_internal(handle);
    return fmt.emitFormatted(backing_allocator, handle.ast_root);
}

/// Lazy `.deal/index.json` emission (D-18). Serializes the symbol table
/// produced by sema into the alphabetical-key shape Phase 3 (LSP) consumes
/// as a workspace symbol index. Runs on first call, caches the bytes in
/// the handle's arena.
///
/// Returns false on NULL handle, missing symbol table (e.g. parse failed
/// before sema ran), or OOM. Caller must clone bytes BEFORE calling
/// `deal_free` (Pitfall 3 — arena-owned pointer).
///
/// EXPORT #9 — added in the Phase 2 closeout to satisfy SPEC criterion #1
/// (`deal check` writes `.deal/index.json`). D-22 counted 8 exports for the
/// in-phase Plans 02-04 + 02-05; this 9th export is a backwards-compatible
/// addition — no existing symbol is removed or renamed.
pub export fn deal_index_json(
    handle_ptr: ?*anyopaque,
    out_ptr: *[*]const u8,
    out_len: *usize,
) callconv(.c) bool {
    const p = handle_ptr orelse return false;
    const h = handleCast(p);
    if (h.cached_index_json == null) {
        const table = h.index_root orelse return false;
        const buf = json.writeIndexJson(
            h.arena.allocator(),
            table,
            "0.1.0-phase2",
        ) catch return false;
        h.cached_index_json = buf;
    }
    const cached = h.cached_index_json.?;
    out_ptr.* = cached.ptr;
    out_len.* = cached.len;
    return true;
}

// lexer is used transitively via json.zig; mark it used.
comptime {
    _ = lexer;
}

test "stub: parse empty source + free does not leak" {
    const source = "";
    const filename = "test.deal";
    const h = deal_parse(source.ptr, source.len, filename.ptr, filename.len);
    try std.testing.expect(h != null);
    try std.testing.expectEqual(@as(u32, 0), deal_diagnostics_count(h));
    try std.testing.expectEqual(false, deal_has_errors(h));

    var json_ptr: [*]const u8 = undefined;
    var json_len: usize = 0;
    try std.testing.expect(deal_ast_json(h, &json_ptr, &json_len));
    const ast_json = json_ptr[0..json_len];
    try std.testing.expect(std.mem.indexOf(u8, ast_json, "\"v\":1") != null);
    try std.testing.expect(std.mem.indexOf(u8, ast_json, "\"mode\":\"deal\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, ast_json, "\"filename\":\"test.deal\"") != null);
    // Empty source produces a deal_file root with no package decl, no defs.
    try std.testing.expect(std.mem.indexOf(u8, ast_json, "\"root\":{") != null);

    deal_free(h);
}

test "deal_check_with_stdlib: NUL-delimited multi-file stdlib seeds every file" {
    // Two stdlib files joined by a NUL (0x00) separator. The dimension `Foo`
    // lives in the SECOND file, so it resolves only if BOTH files are parsed.
    // The previous single-parseFile path treated the concatenation as one file,
    // failed at the second @header/package boundary, and lost all entries.
    const file_a = "package t.a;\nattribute def Alpha { attribute si_M : Integer = 0; attribute si_L : Integer = 0; attribute si_T : Integer = 0; attribute si_I : Integer = 0; attribute si_TH : Integer = 0; attribute si_N : Integer = 0; attribute si_J : Integer = 0; }\n";
    const file_b = "package t.b;\nattribute def Foo { attribute si_M : Integer = 0; attribute si_L : Integer = 2; attribute si_T : Integer = 0; attribute si_I : Integer = 0; attribute si_TH : Integer = 0; attribute si_N : Integer = 0; attribute si_J : Integer = 0; }\n";
    const stdlib_blob = file_a ++ "\x00" ++ file_b;

    // Target uses `Foo` (from the second stdlib file) as an attribute type.
    const target = "package t.user;\npart def Widget { public ( attribute area : Foo [1]; ) }\n";
    const filename = "user.deal";

    var diag_ptr: [*]const u8 = undefined;
    var diag_len: usize = 0;
    const h = deal_check_with_stdlib(
        target.ptr,
        target.len,
        filename.ptr,
        filename.len,
        stdlib_blob.ptr,
        stdlib_blob.len,
        &diag_ptr,
        &diag_len,
    );
    try std.testing.expect(h != null);
    defer deal_free(h);

    // Foo must resolve — no "type not defined" error mentioning it.
    var json_ptr: [*]const u8 = undefined;
    var json_len: usize = 0;
    try std.testing.expect(deal_diagnostics_json(h, &json_ptr, &json_len));
    const diag_json = json_ptr[0..json_len];
    try std.testing.expect(std.mem.indexOf(u8, diag_json, "Foo") == null);
    try std.testing.expectEqual(false, deal_has_errors(h));
}

test "stub: .dealx filename selects dealx mode" {
    const source = "";
    const filename = "model/x.dealx";
    const h = deal_parse(source.ptr, source.len, filename.ptr, filename.len);
    try std.testing.expect(h != null);

    var json_ptr: [*]const u8 = undefined;
    var json_len: usize = 0;
    try std.testing.expect(deal_ast_json(h, &json_ptr, &json_len));
    const ast_json = json_ptr[0..json_len];
    try std.testing.expect(std.mem.indexOf(u8, ast_json, "\"mode\":\"dealx\"") != null);

    deal_free(h);
}
