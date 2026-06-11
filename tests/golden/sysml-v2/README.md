# SysML v2 Golden Fixtures

This directory contains 8 golden fixtures that test the `deal build --target sysml-v2`
pipeline end-to-end (Plan 02-04, REQ #6).

Each fixture is a pair:

| Source file              | Expected output                          |
|--------------------------|------------------------------------------|
| `01-part-def.deal`       | `01-part-def.expected.json`              |
| `02-port-usage.deal`     | `02-port-usage.expected.json`            |
| `03-specialization.deal` | `03-specialization.expected.json`        |
| `04-attribute-usage.deal`| `04-attribute-usage.expected.json`       |
| `05-requirement-def.deal`| `05-requirement-def.expected.json`       |
| `06-trace-link.deal`     | `06-trace-link.expected.json`            |
| `07-package.deal`        | `07-package.expected.json`               |
| `08-dealx-composition.dealx` | `08-dealx-composition.expected.json` |

## Dual-Gate Architecture (REQ #6)

Each fixture is guarded by two independent test gates:

**Gate 1 — Byte-exact match** (`cli/tests/golden_sysml_v2.rs`):
Runs `deal build --target sysml-v2` on the source file, reads the output JSON,
and asserts byte-for-byte equality with the `.expected.json` file.

**Gate 2 — Schema validity on hand-authored files** (`cli/tests/golden_fixture_schema_validity.rs`):
Reads the `.expected.json` file **without invoking the emitter** and validates it
against structural invariants (WARNING-03 mitigation):
- `@id` must be UUID format (`8-4-4-4-12` hyphenated hex).
- `@id` must equal `elementId` (SysON convention).
- `@type` must be a known SysML v2 type.
- All JSON object keys must be alphabetically ordered at every nesting level (D-18).
- All nested elements are recursively validated.

## Why Two Gates?

The byte-exact gate alone has a blind spot: if a developer accidentally commits
a `.expected.json` with a typo (`"PartDefiniton"` instead of `"PartDefinition"`),
`cargo test golden_sysml_v2` will still pass — because the emitter produces the
same broken output and the files are byte-equal.

Gate 2 closes this loop. It fails on the hand-authored file itself, BEFORE the
byte-match can anchor the broken output.

## Updating Fixtures After Intentional Emitter Changes

When the emitter's output format changes intentionally:

1. Run the emitter on each affected source to get fresh output:
   ```sh
   cargo build --bin deal
   for f in tests/golden/sysml-v2/*.deal; do
       stem="${f%.deal}"
       ./target/debug/deal build --target sysml-v2 --output /tmp/out.json "$f"
       cp /tmp/out.json "${stem}.expected.json"
   done
   # Also for .dealx:
   ./target/debug/deal build --target sysml-v2 \
       --output /tmp/out.json tests/golden/sysml-v2/08-dealx-composition.dealx
   cp /tmp/out.json tests/golden/sysml-v2/08-dealx-composition.expected.json
   ```

2. Verify Gate 2 still passes for the updated files:
   ```sh
   cargo test golden_fixture_schema_validity
   ```

3. Verify Gate 1 still passes:
   ```sh
   cargo test golden_sysml_v2
   ```

If Gate 2 fails after updating, the emitter is producing output that violates
structural invariants. Fix the emitter before accepting the new snapshots.

## UUID Stability (D-16 / T-02-15)

The `@id` and `elementId` values are derived via UUID v5 from the qualified path
using a fixed namespace constant. The namespace constant **MUST NOT change** once
any `.expected.json` is committed; changing it would invalidate all golden files.

Namespace constant location: `cli/src/sysml_v2.rs` → `const SYSML_NAMESPACE`.

## Alphabetical Key Order (D-18)

All JSON objects in the emitter output use `serde_json::Map` which is backed by
`BTreeMap` (alphabetical key order). The `preserve_order` feature is **not** enabled
in this project's `Cargo.toml`. Gate 2 verifies this invariant on every `.expected.json`.
