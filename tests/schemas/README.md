# Bundled OMG Schemas

Frozen copies of the OMG SysML v2 and KerML JSON Schemas used by the DEAL
pipeline for offline validation (Plan 02-04 `schema_registry.rs`).

## Provenance

Both schemas come from the OMG release dated **2025-02-01** (`20250201`), as
identified by the `$id` prefix `https://www.omg.org/spec/SysML/20250201/` and
`https://www.omg.org/spec/KerML/20250201/` in the schema definitions.

### SysML.json

- **OMG spec:** SysML v2 (Systems Modeling Language version 2)
- **Release date:** 2025-02-01 (version identifier `20250201`)
- **Source file:** `spec/references/omg-sysml-v2/SysML.json` in this repository
  (git submodule at `spec/`)
- **Upstream URL:** `https://www.omg.org/spec/SysML/20250201/` (OMG SysML 2.0
  final-1.0 release)
- **Lines:** 126,290
- **Schema dialect:** JSON Schema draft-2020-12

### KerML.json

- **OMG spec:** KerML v1 (Kernel Modeling Language version 1)
- **Release date:** 2025-02-01 (version identifier `20250201`)
- **Source file:** `spec/references/omg-kerml-v1/KerML.json` in this repository
- **Upstream URL:** `https://www.omg.org/spec/KerML/20250201/` (OMG KerML v1
  release)
- **Lines:** 42,284
- **Schema dialect:** JSON Schema draft-2020-12

## SHA256

SHA256 digests of the frozen copies. These are computed from the byte-identical
copies in `tests/schemas/` and must match the originals in `spec/references/`.

```
bb0d8af159cf2cbe4a0df4ed6b903505a57e33d047068e5a50a7008a18d546c5  SysML.json
e454fe4b7c04f3d95874b6c1a4e6ef056ea5874c71fd3d17e3180319c2f58ab2  KerML.json
```

To verify:
```bash
shasum -a 256 tests/schemas/SysML.json tests/schemas/KerML.json
```

## Tamper-Detection Invariant

Plan 02-04's `schema_registry.rs` will SHA256-check these bundle files at
validator-construction time and fail loudly on mismatch (T-02-01 mitigation
extension). The digests above are the expected values. If either digest
changes without a deliberate ADR updating this README, validation outcomes
are compromised.

Bumping a schema to a newer OMG release requires:
1. Copying the new schema bytes from `spec/references/`
2. Recomputing the SHA256 and updating this README
3. Opening an ADR documenting the schema version change
4. Accepting that `cargo insta review` will show changed golden fixtures
