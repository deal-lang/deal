---
phase: 04-ecosystem
reviewed: 2026-06-06T00:00:00Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - cli/src/resolver.rs
  - cli/src/reqif.rs
  - cli/src/reqif_schema.rs
  - cli/src/main.rs
  - cli/src/lib.rs
  - cli/tests/resolver_test.rs
  - cli/tests/golden_reqif.rs
  - src/sema.zig
  - src/json.zig
  - scripts/phase-4-smoke.sh
  - scripts/verify-fresh-worktree.sh
  - deal-lang.org/scripts/check-snippets.sh
  - deal-lang.org/scripts/extract_deal_blocks.py
  - tests/golden/reqif/02-trace-relation.expected.reqif
  - tests/regressions/sema/07-dimensional-mismatch.deal
  - tests/regressions/sema/07-conversion-mismatch.deal
  - tests/regressions/sema/07-mixed-unit.deal
  - tests/regressions/sema/07-unknown-unit.deal
  - tests/unit/sema_dimensional.zig
findings:
  critical: 3
  warning: 9
  info: 6
  total: 18
status: issues_found
---

# Phase 4: Code Review Report

**Reviewed:** 2026-06-06
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Summary

Reviewed the phase-4 ecosystem deliverables: the Rust git package resolver
(security gates T-4-02 / T-4-03), the ReqIF 1.2 XML emitter + structural
validator, the Zig dimensional-algebra sema (Check #7), and the supporting
shell/python gate scripts and docs tooling.

The cryptographic tamper-detection (`reqif_schema.rs`), XML escaping (delegated
to quick-xml `Writer`), and the SHA-pinned lock determinism are sound. However,
the review found three BLOCKER-class defects: (1) the path-traversal guard in the
resolver has a `canonicalize()` fallback that silently disarms the second
T-4-02 gate, allowing a relative `path` dep to escape the project root via
`../`; (2) the ReqIF emitter generates `SPEC-OBJECT-REF` targets for trace edges
that point at non-existent SpecObjects (dangling references) and never validates
referential integrity; and (3) the `.reqifz` zip archive is NOT byte-reproducible
despite the D-18 claim, because `SimpleFileOptions::default()` stamps the current
local time into the zip entry, which will break the golden-fixture byte-exact
gate on any machine.

Several warnings concern the spec-relation namespace bug (relations always typed
as "Satisfies" regardless of edge kind), the E2502 unknown-unit false-positive
surface, and dead/contradictory cycle-detection code in the sema.

## Critical Issues

### CR-01: Path-traversal guard defeated by canonicalize() fallback — relative `../` escape

**File:** `cli/src/resolver.rs:337-340`
**Issue:** The second T-4-02 gate is supposed to assert that the fully-resolved
dependency path is not a system path. But the canonicalization falls back to the
*non-canonical* joined path when `canonicalize()` fails (e.g. the target does not
yet exist), and `assert_not_system_path` only rejects a fixed prefix list:

```rust
let resolved = project_dir.join(path);
let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
assert_not_system_path(&canonical)?;
```

`validate_dep_path_lexical` explicitly *accepts* `../sibling` (relative paths are
allowed). A path such as `path = "../../../../../../etc/shadow"` passes the lexical
gate (not absolute), and if the target does not exist `canonicalize()` errors, so
`canonical` becomes the raw `project_dir/../../../../etc/shadow` join. That string
does not start with `/etc` (it starts with the project_dir absolute prefix), so
`assert_not_system_path` passes too. The lock then records an arbitrary
out-of-tree path. The guard's own doc comment (lines 194-200) claims it "catches
symlink chains that resolve to /etc/…", but the fallback path means a
non-existent or symlinked target is never normalized before the prefix check —
the guard is bypassable. CWE-22.

**Fix:** Do not fall back to the un-normalized path. Either require the path to
exist and canonicalize successfully, or normalize lexically (resolve `..`/`.`
components) and then assert the result is still inside the canonicalized
`project_dir`:

```rust
let resolved = project_dir.join(path);
let canonical = resolved
    .canonicalize()
    .with_context(|| format!("path dependency '{}' does not resolve to a real path", path))?;
// Containment check, not just a system-prefix denylist:
if !canonical.starts_with(project_dir) && !is_explicitly_allowed_sibling(&canonical) {
    return Err(anyhow!("path dependency '{}' escapes the project root (T-4-02)", path));
}
assert_not_system_path(&canonical)?;
```

The denylist-only approach in `assert_not_system_path` is the wrong primitive —
prefer an allowlist/containment check.

### CR-02: ReqIF emitter writes dangling SPEC-OBJECT-REF targets; no referential-integrity gate

**File:** `cli/src/reqif.rs:178-187` and `cli/src/reqif_schema.rs:182-296`
**Issue:** SpecRelations are built from any trace edge whose src *or* dst matches
a requirement node. When only one endpoint is a requirement (the common case —
e.g. a `part_def` `satisfies` a `requirement_def`), the emitter still writes
`SOURCE`/`TARGET` `SPEC-OBJECT-REF` elements for *both* endpoints
(`deal_id_to_reqif_id(e.src)` and `...(e.dst)`). The non-requirement endpoint
(e.g. `DEAL_gold_reqif02_trace_Battery`, a `part_def`) is filtered out of
`SPEC-OBJECTS` by the D-59 rule (lines 141-144), so the relation references a
SpecObject that does not exist in the document. The golden fixture's own test
asserts this dangling ref is present (`golden_reqif.rs` / `emit.rs` test
`emit_trace_edge_produces_spec_relation` at reqif.rs:929). Per the ReqIF schema,
`SPEC-OBJECT-REF` must resolve to a declared `SPEC-OBJECT` IDENTIFIER; importing
this into a real ReqIF tool (DOORS, Polarion) will fail or silently drop the
relation. The structural validator (`validate_reqif_xml`) only checks that
elements *have* an IDENTIFIER — it never verifies that REF targets resolve, so
the "hard gate" passes broken output.

**Fix:** Either (a) skip relations whose endpoints are not both emitted
SpecObjects, or (b) add a referential-integrity pass that collects all
`SPEC-OBJECT` IDENTIFIERs and rejects any `SPEC-OBJECT-REF` text that is not in
that set:

```rust
// In emit(): only emit relations where BOTH endpoints are req nodes,
// or emit the non-req endpoint as a minimal SpecObject.
.filter(|e| matches_req(e.src) && matches_req(e.dst))
```

and add to `validate_reqif_xml`: build a `HashSet<String>` of SPEC-OBJECT
IDENTIFIERs and a list of SPEC-OBJECT-REF texts, then push a violation for every
ref with no matching object.

### CR-03: .reqifz archive is not byte-reproducible — default zip timestamp breaks golden gate

**File:** `cli/src/reqif.rs:753-770`
**Issue:** `wrap_in_reqifz` claims D-18 byte-reproducibility ("Uses fixed
FileOptions so the archive is byte-reproducible") but `SimpleFileOptions::default()`
does not pin the entry's last-modified time. The `zip` crate defaults the DOS
timestamp to the current local time (or a build-dependent default), so the bytes
of `model.reqifz` differ run-to-run / machine-to-machine. The golden test
(`golden_reqif.rs`) sidesteps this by extracting and comparing only the inner XML,
so the broken reproducibility is invisible to CI — but any consumer that hashes or
diffs the `.reqifz` itself (the actual D-61 artifact) gets non-deterministic
output, defeating the stated determinism contract and any supply-chain pinning of
the archive.

**Fix:** Explicitly pin the modification time to a fixed epoch:

```rust
let options = zip::write::SimpleFileOptions::default()
    .compression_method(zip::CompressionMethod::Deflated)
    .last_modified_time(zip::DateTime::from_date_and_time(2026, 1, 1, 0, 0, 0)
        .expect("valid fixed timestamp"))
    .unix_permissions(0o644);
```

(Match the `FIXED_CREATION_TIME` epoch used for the XML header.) Add a test that
asserts two `wrap_in_reqifz` calls produce byte-identical archives.

## Warnings

### WR-01: All SpecRelations typed as "Satisfies" regardless of edge kind

**File:** `cli/src/reqif.rs:177, 198, 298-306, 394-405`
**Issue:** The emitter accepts four trace kinds (`satisfies`, `traces`,
`derives_from`, `allocated_to`) but defines exactly one `SPEC-RELATION-TYPE`
(`SatisfiesRelation`) and points every relation's `SPEC-RELATION-TYPE-REF` at
`srt_satisfies_id`. A `derives_from` or `allocated_to` edge is exported as a
"Satisfies" relation — semantic data loss. The doc comment in the file header
(D-59) explicitly lists distinct edge kinds, so collapsing them is a defect, not
a documented simplification.

**Fix:** Emit one `SPEC-RELATION-TYPE` per distinct trace kind and select the REF
by `e.kind`, or at minimum carry the original kind in the relation's `LONG-NAME`
so it is not silently lost.

### WR-02: E2402 not-installed guard only fires when a directory arg is passed

**File:** `cli/src/main.rs:173-207`
**Issue:** `run_check` looks for `deal.toml` under the first directory argument,
or `.` if none. When the user runs `deal check packages/foo.deal` (a file arg,
no directory), `paths.iter().find(|p| p.is_dir())` is `None`, so `check_root`
becomes `.`. If the cwd is not the project root, the deal.toml / not-installed
check is silently skipped, and a build that should have been blocked by E2402
proceeds. The behavior depends on cwd and on whether the user happened to pass a
directory — surprising and inconsistent.

**Fix:** Resolve the project root by walking up from each input path looking for
`deal.toml` (as `infer_output_path` already does for `Cargo.toml`), rather than
relying on a directory argument being present.

### WR-03: Unknown-unit (E2502) false positive for any non-unit function call in a unit context

**File:** `src/sema.zig:1128-1143`
**Issue:** `checkCallDimension` emits E2502 ("unknown unit") for *any* callee that
is not in the symbol table, whenever a `call` expression appears as an attribute
default value. A DEAL attribute initialized with a legitimate non-unit function
call (e.g. a computed default `max(a, b)` or a constructor that is not a unit)
will be flagged as an "unknown unit" even though it is not a unit at all. The
check cannot distinguish "unknown unit" from "ordinary call expression" because
there is no notion of which identifiers are unit constructors vs. functions. This
will produce spurious E2502 diagnostics on valid models once attribute defaults
beyond unit literals are used.

**Fix:** Only emit E2502 when the surrounding context is dimensional (the declared
type resolves to a dimension) AND the callee name matches a unit-naming
convention, or maintain an explicit set of unit constructor names from the stdlib
seed and gate E2502 on membership-miss within that namespace rather than the whole
symbol table.

### WR-04: Specialization cycle detection is effectively dead for indirect cycles

**File:** `src/sema.zig:955-1002`
**Issue:** `checkSpecializationCycle` only ever detects the *direct*
`target == current` self-reference (lines 964-973). The `visited` slice is always
passed as `&[_][]const u8{}` from the single call site (line 805) and is never
appended to, so the loop at lines 974-985 never matches. The function looks up
`target_entry` (line 990) and then does nothing with it (the comment at lines
994-1001 admits "we cannot follow the chain further"). An indirect cycle
`A specializes B`, `B specializes A` is NOT detected unless the direct
self-reference path happens to fire. This contradicts the stated check #4 ("forms
no cycle") and the T-02-08 DoS-mitigation claim. The `visited.len > count()` bound
(line 987) and the `target_entry`/`if .imported` block (lines 990-992) are dead
code.

**Fix:** Store each symbol's `specializes` target in `SymbolEntry` during Pass A,
then make `checkSpecializationCycle` actually recurse through the chain, appending
`current_name` to `visited` on each hop. Until then, document the limitation
honestly and remove the dead branches.

### WR-05: Duplicate-declaration check keys on bare name, missing cross-package duplicates and producing wrong location

**File:** `src/sema.zig:447-458, 491-494`
**Issue:** The E2002 duplicate check (line 447) looks up `elem.name` (the bare,
unqualified name). Two definitions with the same bare name in different scopes
collide and are reported as duplicates even when their qualified IDs differ; and
the error message reports `existing.span.start`/`.end` as `"{d}:{d}"` (line 453),
which are byte offsets, not line:column — the message claims a `line:col`-style
"first declared at" location but prints raw spans. Additionally both the bare name
and qualified id are inserted pointing at the same entry (lines 493-494), so a
later definition with the same bare name in a different package silently
overwrites the qualified entry for the first.

**Fix:** Key the duplicate check on the qualified `id`, and convert the span to a
line:column pair (or relabel the message to "first declared at byte offset N").

### WR-06: `isTraceAnnotation` has a tautological / dead second clause

**File:** `src/sema.zig:1350-1353`
**Issue:**

```zig
return std.mem.eql(u8, ann.name, "trace") or
    (std.mem.eql(u8, ann.name, "trace") and ann.operator != null);
```

The right operand of the `or` is a strict subset of the left (`name == "trace"`
is already covered), so the entire second clause is dead. If the intent was to
also match an operator-bearing variant with a *different* name (e.g. the
`<<satisfies>>` form), this is a logic bug that fails to recognize those
annotations.

**Fix:** Confirm the intended trace-annotation forms and rewrite, e.g.
`std.mem.eql(u8, ann.name, "trace") or std.mem.eql(u8, ann.name, "satisfies")`,
or drop the redundant clause.

### WR-07: FS-3 identity gate silently passes when git is absent or returns whitespace

**File:** `cli/src/main.rs:577-593`
**Issue:** `check_fs3_identity` treats a missing `git` binary as success (line
586-591, returns `Ok(())`), and treats any non-empty stdout as a valid name —
including a name consisting solely of a trailing newline / whitespace
(`!output.stdout.is_empty()` is true for `"\n"`). The gate is meant to enforce a
committer identity (FS-3); both paths let an unidentified user through. The
"missing git = OK" branch makes the gate trivially bypassable by running outside a
repo.

**Fix:** Trim stdout and require a non-empty trimmed value; decide explicitly
whether absent-git should fail closed (recommended for a provenance gate) rather
than defaulting to OK.

### WR-08: ReqIF structural depth/`inside_core_content` tracking is order-fragile

**File:** `cli/src/reqif_schema.rs:140-142, 227-250`
**Issue:** `found_spec_objects` is only set when `SPEC-OBJECTS` is seen while
`inside_core_content` is true, but `inside_core_content` is cleared on the
`CORE-CONTENT` End event by name match only — there is no depth comparison
against `core_content_depth` (which is computed but explicitly discarded at line
258 with `let _ = core_content_depth;`). A nested element also named
`CORE-CONTENT` (legal XML, even if unusual) would prematurely clear the flag.
More importantly, for `Event::Empty` the code increments then decrements depth
(lines 231/249) but still calls `process_start_element` with the incremented
depth, so a self-closing `<CORE-CONTENT/>` sets `inside_core_content = true`
permanently (no matching End event ever clears it). The whole depth apparatus is
half-wired dead code.

**Fix:** Either remove the depth tracking entirely (it is unused) or finish it:
compare End depth to `core_content_depth`, and handle the self-closing
`CORE-CONTENT` case.

### WR-09: SYSTEM_PATH_PREFIXES denylist is incomplete and substring-prone

**File:** `cli/src/resolver.rs:147-151, 201-213`
**Issue:** `assert_not_system_path` uses `resolved_str.starts_with(prefix)` with
prefixes like `/etc`, `/var`. This both (a) misses sibling-sensitive system dirs
not in the list (`/Library`, `/System`, `/Users/<other>`, Windows `C:\Windows`),
and (b) false-positively matches legitimate paths like `/etcetera-project` or
`/usrlocal` because the prefix is not boundary-anchored. Combined with CR-01 this
denylist is the wrong security primitive; even as a "sanity guard" it is
misleading.

**Fix:** Anchor prefixes on a path separator (`/etc/` or exact `/etc`), and
replace the denylist with the containment check described in CR-01.

## Info

### IN-01: Misleading comment — `to_kg` "converts to {declared_type_name}"

**File:** `src/sema.zig:1169-1180`
**Issue:** The E2503 message interpolates `declared_type_name` to describe what
the conversion "converts to", but `declared_type_name` is the attribute's declared
type, not the conversion target — for `attribute x : Mass = to_kg(V(800))` the
message happens to read "Mass" only because the attribute is Mass-typed. With a
mismatched declared type the message is wrong.
**Fix:** Derive the conversion's own target dimension name for the message.

### IN-02: Dead / unused `ReqifSchemas` bytes

**File:** `cli/src/reqif_schema.rs:52-59`
**Issue:** `reqif_xsd` / `driver_xsd` are `#[allow(dead_code)]` and never consumed
— the SHA-verified bytes are loaded and discarded. This is acknowledged but the
whole `load_reqif_schemas` path is unused by `validate_reqif_xml` (the actual
gate), so the tamper-detection never runs in the build pipeline unless a test
calls it.
**Fix:** Wire `load_reqif_schemas` into the emit path (call it before
`validate_reqif_xml`) so T-4-12 tamper-detection actually executes in production,
or document that it is test-only.

### IN-03: `count_spec_objects` / `count_spec_relations` substring counting is brittle

**File:** `cli/src/reqif.rs:736-747`
**Issue:** Requirement/relation counts are computed by byte-window scanning for
`"<SPEC-OBJECT "` / `"<SPEC-RELATION "`. `"<SPEC-RELATION "` is also a prefix of
`"<SPEC-RELATION-TYPE "`... but note the type element is emitted as
`<SPEC-RELATION-TYPE` (no trailing space before the hyphen), so it is not matched
— fragile but currently correct. Any future formatting change (attribute on its
own line) silently corrupts the user-facing counts.
**Fix:** Return the counts from `emit()` directly (`spec_objects.len()`,
`spec_relations.len()`) instead of re-parsing the serialized XML.

### IN-04: `extract_deal_blocks.py` closing-fence regex ignores indentation

**File:** `deal-lang.org/scripts/extract_deal_blocks.py:80, 107`
**Issue:** The closing-fence match `^`{N,}\s*$` requires the fence at column 0.
Indented fenced blocks (valid inside MDX list items / admonitions) will never
close, causing the extractor to swallow the rest of the file into one snippet and
mis-report parse failures.
**Fix:** Allow leading whitespace before the closing fence and track the opening
fence's indentation.

### IN-05: phase-4-smoke.sh asserts `grep -q 'git'` which over-matches

**File:** `scripts/phase-4-smoke.sh:79`
**Issue:** `grep -q 'git'` on deal.toml matches the substring "git" anywhere
(e.g. inside a URL `github.com`), not the `git = ` dependency key specifically.
The assertion can pass even if the dependency key is malformed.
**Fix:** `grep -qE '^\s*git\s*=' "$PROJ_DIR/deal.toml"` or grep for the inline
table form.

### IN-06: `app.ini.bak` / `.idea/` committed-adjacent artifacts present in working tree

**File:** (repo root, not in review scope but observed) `app.ini.bak`, `.idea/`
**Issue:** Untracked backup/editor-config artifacts are present in the working
tree. Not part of this phase's source but worth a `.gitignore` entry to avoid
accidental commits.
**Fix:** Add `*.bak` and `.idea/` to `.gitignore`.

---

_Reviewed: 2026-06-06_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
