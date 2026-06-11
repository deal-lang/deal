# ReqIF Golden Fixtures

This directory contains golden fixtures for the `deal build --target reqif` pipeline
(Plan 04-05, REQ-phase-4-2-reqif-codegen).

Each fixture is a pair:

| Source file                       | Expected output                                 |
|-----------------------------------|-------------------------------------------------|
| `01-requirement-def.deal`         | `01-requirement-def.expected.reqif`             |
| `02-trace-relation.deal`          | `02-trace-relation.expected.reqif`              |

## Fixture Convention

**`.deal` input file** — a valid DEAL source file that exercises a specific ReqIF
mapping pattern. Must parse cleanly (`deal parse` exits 0). The file exercises only
sema-valid constructs so semantic errors do not interfere with the emitter.

**`.expected.reqif` expected output** — hand-authored raw ReqIF XML (not compressed).
The test runner invokes `deal build --target reqif` on the input, extracts the
`reqif.reqif` entry from the emitted `.reqifz` archive, and diffs the XML against
this file byte-for-byte.

During Wave 0 (scaffold phase), the `.expected.reqif` files contain placeholder
comments only. The real expected XML is authored in Plan 04-05 once the emitter
is implemented and its output is verified against the OMG ReqIF 1.2 XSD
(`spec/references/omg-reqif/reqif.xsd`).

## Test Runner

Golden fixture tests live in `cli/tests/golden_reqif.rs` (created in Plan 04-05).
The runner:

1. Runs `deal build --target reqif --output /tmp/out.reqifz <input>.deal`
2. Extracts `reqif.reqif` from the `.reqifz` ZIP archive
3. Diffs the extracted XML against `<input>.expected.reqif`
4. Validates the extracted XML against `spec/references/omg-reqif/reqif.xsd`
   (the XSD validation gate per D-60)

## Updating Fixtures After Intentional Emitter Changes

When the emitter's output changes intentionally:

```sh
cargo build --bin deal
for f in tests/golden/reqif/*.deal; do
    stem="${f%.deal}"
    ./target/debug/deal build --target reqif --output /tmp/out.reqifz "$f"
    # Extract the reqif.reqif entry and update the expected file
    unzip -p /tmp/out.reqifz reqif.reqif > "${stem}.expected.reqif"
done
```

Then verify XSD validity:

```sh
cargo test golden_reqif
```

## XSD Validation Gate (D-60, T-4-01)

Every `.expected.reqif` file is validated against
`spec/references/omg-reqif/reqif.xsd` (OMG ReqIF 1.2, formal/2016-07-01)
before any byte-comparison, ensuring the emitter never produces schema-invalid XML.
The SHA256SUMS manifest in `spec/references/omg-reqif/` pins the XSD bytes
against tampering (T-4-SC, T-4-01 mitigations).
