/**
 * @file deal.h
 * @brief DEAL compiler C ABI — hand-written header (not -femit-h generated).
 *
 * This header exposes the seven C ABI functions of the DEAL compiler Zig
 * core (`libdeal.a`) after Plan 02-03. The surface is intentionally narrow:
 * parse source bytes, read back structured results (AST JSON, diagnostic
 * JSON, IR JSON), and free the handle. File I/O and higher-level
 * orchestration live in the Rust CLI / LSP server that links this library.
 * Plan 02-04 adds deal_format (D-21) → 8 exports. Phase 2 closeout adds
 * deal_index_json (SPEC §criterion 1) → 9 exports.
 *
 * @section ownership Ownership Model (D-11)
 * Handles are owned by the caller and freed via deal_free(). Buffer pointers
 * returned by deal_ast_json() and deal_diagnostics_json() are owned by the
 * handle's arena and become invalid after deal_free(). Callers must copy the
 * buffer contents before calling deal_free() if they need the data to outlive
 * the handle.
 *
 * @section thread_model Thread Model (D-13)
 * Different handles may be used concurrently from different threads — each
 * deal_parse() allocates a fresh independent arena with no shared state.
 * A single handle is NOT thread-safe; if a single handle is accessed from
 * multiple threads, the caller must serialize access externally.
 *
 * @section error_model Error Model (D-10)
 * deal_parse() always returns a non-null handle except on allocator failure
 * (out-of-memory). Parse errors are reported through the handle via
 * deal_has_errors() / deal_diagnostics_count() / deal_diagnostics_json().
 * A non-null handle with deal_has_errors() == true contains a partial or
 * empty AST (ast_root may be null in the JSON output).
 *
 * @section string_model String / Buffer Model (D-11)
 * All output buffers are UTF-8, length-prefixed (out_ptr + out_len pairs).
 * Buffers are NOT NUL-terminated. Callers must use the length field to
 * determine buffer extent.
 *
 * @section error_codes Error Code Reference (D-16)
 * @code
 * Range       | Category
 * ------------|--------------------------------------------------
 * E2001-E2099 | IR lowering / emission errors
 *   E2001     | IR lowering failed (OOM or no AST root)
 * E0001-E0099 | Lexer errors
 *   E0001     | Invalid UTF-8 in source bytes
 *   E0002     | Unterminated string literal
 *   E0003     | Unterminated block comment
 *   E0004     | Source too large (> 4 GiB; spans are u32)
 *   E0005     | Template literal nesting too deep
 * E0100-E0299 | Parser errors (.deal definition files)
 *   E0100     | Expected token (unexpected token found)
 *   E0101     | Expected definition keyword
 *   E0102     | Unexpected end of file
 *   E0103     | Expected attribute value
 *   E0110     | Expected expression
 *   E0111     | Expected identifier
 *   E0120     | Invalid modifier
 * E0300-E0399 | Parser errors (.dealx composition files)
 *   E0301     | Unmatched close tag [</name>] with no open
 *   E0302     | Mismatched close tag (opened X, closed Y)
 *   E0303     | Tag nesting too deep
 *   E0304     | Unclosed tag at end of file
 * E0400-E0499 | Recovery / structural errors
 *   E0400     | Sync: dropped tokens during error recovery
 *   E0401     | Unclosed brace
 *   E0402     | Unclosed bracket
 *   E0403     | Expression nesting too deep (> 256 levels)
 * W0500-W0599 | Warnings
 *   W0500     | Unused import
 * H0600-H0699 | Hints
 *   H0600     | Did-you-mean suggestion
 * @endcode
 *
 * @version 0.1.0-draft (Phase 1)
 * @see deal/src/lib.zig — Zig implementation
 * @see deal/.planning/phases/01-zig-compiler-core/01-CONTEXT.md — D-10..D-18
 */

#ifndef DEAL_H
#define DEAL_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Opaque parse handle returned by deal_parse().
 *
 * The internal layout is private to the Zig library. Callers treat this as a
 * forward-declared struct pointer. Allocate via deal_parse(), free via
 * deal_free(). Do not access fields directly.
 *
 * @note The handle is NOT thread-safe. Do not share a single handle across
 * threads. Different handles in different threads are fully independent.
 */
typedef struct DealHandle DealHandle;

/**
 * @brief Parse DEAL source bytes and return a handle carrying the result.
 *
 * Parses @p source_len bytes starting at @p source_ptr. The file mode
 * (.deal vs .dealx) is determined by the filename extension (D-12):
 * filenames ending in ".dealx" select composition mode; all other filenames
 * (including ".deal") select definition mode.
 *
 * The caller's source and filename buffers are duped into the handle's arena
 * before this function returns. The caller may free or mutate the original
 * buffers immediately after deal_parse() returns.
 *
 * Security controls applied before invoking the parser:
 * - source_len > UINT32_MAX: emits E0004, returns handle (no parse).
 * - Invalid UTF-8 bytes: emits E0001, returns handle (no parse).
 *
 * @param source_ptr  Pointer to source bytes (need not be NUL-terminated).
 *                    May be NULL if @p source_len is 0. If @p source_len is
 *                    greater than 0 and @p source_ptr is NULL, this function
 *                    returns NULL (input validation, CR-01).
 * @param source_len  Length in bytes of the source buffer.
 * @param filename_ptr Pointer to filename bytes (UTF-8, not NUL-terminated).
 *                    Used to select parse mode and populate diagnostic spans.
 *                    May be NULL if @p filename_len is 0. If @p filename_len
 *                    is greater than 0 and @p filename_ptr is NULL, this
 *                    function returns NULL (input validation, CR-01).
 * @param filename_len Length in bytes of the filename buffer.
 *
 * @returns Non-null handle on success. Returns NULL on (a) allocator failure
 *          (out-of-memory), or (b) NULL pointer paired with a non-zero length
 *          on either buffer (caller misuse). Parse errors are reported through
 *          the handle via deal_has_errors() / deal_diagnostics_json().
 *
 * @note Ownership: the returned handle is owned by the caller. Free it with
 *       deal_free() when done.
 *
 * @thread_safety Safe to call concurrently from multiple threads. Each call
 *                returns an independent handle with its own arena. The
 *                returned handle is NOT thread-safe; serialize access if
 *                shared across threads (D-13).
 */
DealHandle* deal_parse(
    const uint8_t* source_ptr,
    size_t         source_len,
    const uint8_t* filename_ptr,
    size_t         filename_len
);

/**
 * @brief Release every byte allocated during deal_parse().
 *
 * Frees the handle's arena (AST nodes, duped source bytes, duped filename,
 * diagnostic strings, cached JSON buffers). After this call, @p handle and
 * any pointers previously obtained via deal_ast_json() or
 * deal_diagnostics_json() are invalid.
 *
 * @param handle  Handle to free. Safe to call with NULL (no-op, matches
 *                free() C semantics).
 *
 * @note Ownership: the caller must not access @p handle after this call.
 *
 * @thread_safety Not thread-safe for a single handle. Do not call deal_free()
 *                concurrently with any other operation on the same handle.
 */
void deal_free(DealHandle* handle);

/**
 * @brief Query whether the handle carries any diagnostics.
 *
 * Returns true if the handle has one or more diagnostics (any severity).
 * Use deal_diagnostics_json() to retrieve the full diagnostic details.
 *
 * @param handle  Handle to query. Returns false for NULL.
 *
 * @returns true if any diagnostics are present; false otherwise or if
 *          @p handle is NULL.
 *
 * @thread_safety Not thread-safe for a single handle.
 */
bool deal_has_errors(DealHandle* handle);

/**
 * @brief Return the number of diagnostics on the handle.
 *
 * Counts diagnostics of all severities (errors, warnings, hints).
 *
 * @param handle  Handle to query. Returns 0 for NULL.
 *
 * @returns Diagnostic count; 0 if @p handle is NULL.
 *
 * @thread_safety Not thread-safe for a single handle.
 */
uint32_t deal_diagnostics_count(DealHandle* handle);

/**
 * @brief Emit the AST as UTF-8 JSON and write the buffer location.
 *
 * On the first call, the JSON is generated lazily and cached inside the
 * handle's arena. Subsequent calls return the same (ptr, len) pair —
 * the buffer is immutable after first generation.
 *
 * The JSON schema (D-04, v1):
 * @code
 * {
 *   "v": 1,
 *   "mode": "deal" | "dealx",
 *   "filename": "<supplied filename>",
 *   "root": <node object> | null
 * }
 * @endcode
 *
 * Each AST node object has shape:
 * @code { "k": "<kind>", "span": [start, end], ... kind-specific fields } @endcode
 *
 * All object fields are emitted in alphabetical order (D-18).
 *
 * @param handle   Handle from deal_parse(). Returns false for NULL.
 * @param out_ptr  Output: written with the start of the JSON buffer.
 * @param out_len  Output: written with the byte length (no NUL terminator).
 *
 * @returns true on success; false on NULL handle or allocator failure.
 *          On false, @p out_ptr and @p out_len are not written.
 *
 * @note Ownership: the buffer is owned by the handle's arena and freed by
 *       deal_free(). Valid until deal_free() is called.
 *
 * @note Calling deal_ast_json() twice on the same handle returns identical
 *       (ptr, len) — same pointer, same bytes (D-11 caching guarantee).
 *
 * @thread_safety Not thread-safe for a single handle.
 */
bool deal_ast_json(
    DealHandle*     handle,
    const uint8_t** out_ptr,
    size_t*         out_len
);

/**
 * @brief Emit diagnostics as a UTF-8 JSON array and write the buffer location.
 *
 * Same lazy + cached generation contract as deal_ast_json(). The output is a
 * JSON array (may be empty `[]` if there are no diagnostics). All fields
 * within each diagnostic object are emitted in alphabetical order (D-18).
 *
 * Each diagnostic object has this shape (fields alphabetical):
 * @code
 * {
 *   "code": "<E/W/H code>",
 *   "fix_it": null | { "replace_span": [start, end], "replacement": "<text>" },
 *   "message": "<human-readable message>",
 *   "notes": "<optional note text>",
 *   "secondary_spans": [ { "label": "...", "span": [start, end] }, ... ],
 *   "severity": "err" | "warn" | "info" | "hint",
 *   "span": [start, end]
 * }
 * @endcode
 *
 * Span values are u32 byte offsets into the original source buffer.
 *
 * @param handle   Handle from deal_parse(). Returns false for NULL.
 * @param out_ptr  Output: written with the start of the JSON buffer.
 * @param out_len  Output: written with the byte length (no NUL terminator).
 *
 * @returns true on success; false on NULL handle or allocator failure.
 *          On false, @p out_ptr and @p out_len are not written.
 *
 * @note Ownership: the buffer is owned by the handle's arena and freed by
 *       deal_free(). Valid until deal_free() is called.
 *
 * @note Calling deal_diagnostics_json() twice returns identical (ptr, len)
 *       — same pointer, same bytes (D-11 caching guarantee).
 *
 * @thread_safety Not thread-safe for a single handle.
 */
bool deal_diagnostics_json(
    DealHandle*     handle,
    const uint8_t** out_ptr,
    size_t*         out_len
);

/**
 * @brief Emit the IR Document as UTF-8 JSON and write the buffer location.
 *
 * Runs the lowering pass (AST + symbol table → IR Document) on first call,
 * then serializes the result to JSON conforming to spec/ir/v0/schema.json.
 * The result is cached inside the handle's arena; subsequent calls return
 * the same (ptr, len) pair — the buffer is immutable after first generation.
 *
 * The JSON envelope (D-22, D-18 alphabetical keys):
 * @code
 * {
 *   "edges":    [ { "dst": "...", "kind": "...", "src": "..." }, ... ],
 *   "elements": {
 *     "<qualified.path>": {
 *       "kind":        "<NodeKind>",
 *       "payload":     { ... alphabetical ... },
 *       "source_file": "<workspace-relative path>",
 *       "span":        [<start_u32>, <end_u32>]
 *     }
 *   },
 *   "ir_version": "v0",
 *   "v": 1
 * }
 * @endcode
 *
 * @param handle   Handle from deal_parse(). Returns false for NULL.
 * @param out_ptr  Output: written with the start of the JSON buffer.
 * @param out_len  Output: written with the byte length (no NUL terminator).
 *
 * @returns true on success; false on NULL handle, OOM, or if the handle
 *          carries no parsed AST (e.g. invalid UTF-8 or source too large).
 *          On false, @p out_ptr and @p out_len are not written.
 *
 * @note Ownership: the buffer is owned by the handle's arena and freed by
 *       deal_free(). Valid until deal_free() is called.
 *
 * @note Calling deal_ir_json() twice on the same handle returns identical
 *       (ptr, len) — same pointer, same bytes (D-11 caching guarantee).
 *
 * @note IR is produced even when sema diagnostics are present (partial
 *       models are useful for downstream consumers that tolerate unresolved
 *       references).
 *
 * @thread_safety Not thread-safe for a single handle.
 *
 * @see spec/ir/v0/schema.json — normative JSON Schema (D-27)
 * @see spec/ir/v0/README.md  — reference documentation (D-27)
 */
bool deal_ir_json(
    DealHandle*     handle,
    const uint8_t** out_ptr,
    size_t*         out_len
);

/**
 * @brief Emit canonical formatted DEAL source bytes and write the buffer location.
 *
 * Runs the AST pretty-printer (src/fmt.zig) on first call, then caches the
 * result inside the handle's arena. Subsequent calls return the same
 * (ptr, len) pair — the buffer is immutable after first generation.
 *
 * The output is canonical DEAL source text conforming to the round-trip
 * invariants (Plan 02-05, D-21):
 *   1. One space around binary operators
 *   2. Four-space indent (no tabs)
 *   3. One blank line between top-level declarations
 *   4. Single-quoted strings normalized to double-quoted (LM-3)
 *   5. No trailing commas (E0122 guard)
 *   6. Comments preserved verbatim at original attachment points (D-28, D-29)
 *
 * Output format: UTF-8 source bytes (NOT JSON). This is NOT the D-32 envelope.
 * The D-32 envelope wraps diagnostics when `--json` is set; the formatted
 * source goes directly to stdout or in-place to the file.
 *
 * @param handle   Handle from deal_parse(). Returns false for NULL.
 * @param out_ptr  Output: written with the start of the formatted-source buffer.
 * @param out_len  Output: written with the byte length (no NUL terminator).
 *
 * @returns true on success; false on NULL handle or allocator failure.
 *          On false, @p out_ptr and @p out_len are not written.
 *
 * @note Ownership: the buffer is owned by the handle's arena and freed by
 *       deal_free(). Valid until deal_free() is called.
 *
 * @note Pitfall 3 (T-02-29): Rust CLI MUST clone formatted bytes (Cow or
 *       Vec::from(slice)) BEFORE calling deal_free(). Using the raw pointer
 *       after deal_free() is undefined behavior.
 *
 * @note Calling deal_format() twice on the same handle returns identical
 *       (ptr, len) — same pointer, same bytes (D-11 caching guarantee).
 *
 * @note Formatter walks AST only (D-25) — IR is not required or touched.
 *       deal_format() can be called even when deal_ir_json() would fail
 *       (e.g. if sema produced no symbol table), because the formatter
 *       does not depend on the IR.
 *
 * @thread_safety Not thread-safe for a single handle.
 *
 * @see D-21 — Zig owns the pretty-printer; Rust CLI calls via FFI
 * @see D-25 — IR is comment-free; formatter walks AST
 * @see D-28, D-29 — comment attachment fields on ElementDef / ElementUsage
 */
bool deal_format(
    DealHandle*     handle,
    const uint8_t** out_ptr,
    size_t*         out_len
);

/**
 * Emit `.deal/index.json` bytes for the workspace.
 *
 * Serializes the sema symbol table into the alphabetical-key shape Phase 3
 * (LSP) consumes as a workspace symbol index. Lazy: the first call runs the
 * serializer and caches the result on the handle; subsequent calls return
 * the same (ptr, len) pair (D-11 caching guarantee).
 *
 * @param handle   Non-NULL handle from deal_parse.
 * @param out_ptr  Out: pointer to UTF-8 JSON bytes (arena-owned).
 * @param out_len  Out: length in bytes.
 *
 * @return true on success; false if handle is NULL, sema did not run
 *         (parse failed before sema), or allocation fails.
 *
 * @warning Caller MUST clone the bytes BEFORE calling deal_free() —
 *          the arena that owns these bytes is destroyed at deal_free().
 *
 * @note Calling deal_index_json() twice on the same handle returns the
 *       identical (ptr, len) pair (D-11 caching).
 *
 * @thread_safety Not thread-safe for a single handle.
 *
 * @see D-18 — alphabetical-key shape
 * @see SPEC §criterion 1 — `deal check tests/showcase/` writes .deal/index.json
 * @see Export #9 — backwards-compatible Phase 2 closeout addition
 */
bool deal_index_json(
    DealHandle*     handle,
    const uint8_t** out_ptr,
    size_t*         out_len
);

/**
 * @brief Analyze DEAL source bytes with a stdlib seed and emit diagnostics JSON.
 *
 * Phase 5 C ABI export #10 (D-85, D-88). Exposes `analyzeWithExternalTable`
 * across the C ABI boundary so the Rust orchestrator can check a DEAL file
 * with the stdlib dimension/unit table seeded without re-parsing stdlib source
 * from the Rust side.
 *
 * @param source_ptr     Pointer to DEAL source bytes (UTF-8). NULL if source_len == 0.
 * @param source_len     Length of source buffer in bytes.
 * @param filename_ptr   Pointer to filename bytes. NULL if filename_len == 0.
 * @param filename_len   Length of filename buffer in bytes.
 * @param stdlib_ir_ptr  Pointer to stdlib DEAL source bytes (UTF-8). Used to build
 *                       the external dimension/unit symbol table before analysis.
 *                       NULL if stdlib_ir_len == 0 (analysis proceeds with no stdlib seed).
 * @param stdlib_ir_len  Length of stdlib source buffer in bytes.
 * @param out_diag_ptr   Output: written with the start of the diagnostics JSON buffer.
 * @param out_diag_len   Output: written with the byte length (no NUL terminator).
 *
 * @returns Non-null opaque handle on success; NULL on allocator failure or
 *          NULL pointer paired with non-zero length on any buffer (caller misuse).
 *          Caller MUST call deal_free() on the returned handle to release the arena.
 *          Diagnostics (has_errors) can also be queried via deal_has_errors().
 *
 * @note Ownership: the returned handle is owned by the caller. Caller MUST call
 *       deal_free() when done. The out_diag_ptr/out_diag_len buffer is arena-owned
 *       and becomes invalid after deal_free(). Clone bytes BEFORE calling deal_free()
 *       (Pitfall 3 / T-02-29).
 *
 * @note Security: NULL+non-zero length pairs are rejected at entry (ASVS V5,
 *       T-05-01). stdlib_ir parse failure is silently skipped (T-05-02). Source
 *       larger than 4 GiB emits E0004. Invalid UTF-8 emits E0001.
 *
 * @thread_safety Not thread-safe for a single handle.
 *
 * @see D-85 — Rust orchestrates; Zig owns dimensions
 * @see D-88 — E2500 CLI carryover wiring
 */
DealHandle* deal_check_with_stdlib(
    const uint8_t*  source_ptr,
    size_t          source_len,
    const uint8_t*  filename_ptr,
    size_t          filename_len,
    const uint8_t*  stdlib_ir_ptr,
    size_t          stdlib_ir_len,
    const uint8_t** out_diag_ptr,
    size_t*         out_diag_len
);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* DEAL_H */
