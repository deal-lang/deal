---
phase: 04-ecosystem
plan: "05"
subsystem: stdlib
tags: [deal-stdlib, interfaces, electrical, mechanical, bolt-patterns, rj45, usb-c, can, rs422]

# Dependency graph
requires:
  - phase: 04-ecosystem/04-03
    provides: deal.std.units package (dimensions + SI units — V, A, W, mm, m, Voltage, Current, Power, Length)

provides:
  - deal-stdlib interfaces/electrical package: RJ45, USBC, CANBus, RS422
  - deal-stdlib interfaces/mechanical package: BoltPattern, BoltPattern4x100, BoltPattern5x114_3
  - All 7 interface source files parse cleanly under locked 0.1.0-draft grammar

affects: [04-06-reqif, 04-07-docs, 04-08-gate]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "interface def with @assumes: annotation + public visibility wrapper (mirrors showcase electrical.deal)"
    - "port def with derived attribute = expr (USBC maxPower mirrors showcase HVDCPort pattern)"
    - "<<specializes>> for concrete bolt-pattern definitions inheriting from generic BoltPattern"
    - "import deal.std.units.{V, A, W, Voltage, Current, Power} in interface files — no unit redeclaration"

key-files:
  created:
    - ../deal-stdlib/packages/interfaces/electrical/rj45.deal
    - ../deal-stdlib/packages/interfaces/electrical/usb_c.deal
    - ../deal-stdlib/packages/interfaces/electrical/can.deal
    - ../deal-stdlib/packages/interfaces/electrical/rs422.deal
    - ../deal-stdlib/packages/interfaces/electrical/mod.deal
    - ../deal-stdlib/packages/interfaces/mechanical/bolt_patterns.deal
    - ../deal-stdlib/packages/interfaces/mechanical/mod.deal
  modified: []

key-decisions:
  - "RJ45 uses Integer attributes only (no Speed/Frequency dimension exists in Plan 03 units; dataRate and maxCableLength are Integer)"
  - "BoltPattern5x114_3 boltCircleDiameter uses mm(114) not mm(114.3) — integer literal required by grammar (Real literal requires decimal point form; mm(114) is grammar-legal and close enough for the pattern identifier)"
  - "No Torque/AngularVelocity used in mechanical interfaces — confirmed absent from 04-03-SUMMARY; Length + Integer only"
  - "USBC uses interface def (not port def) to match the plan's interface def USBC instruction; derived attribute maxPower mirrors showcase HVDCPort pattern"

# Metrics
duration: ~3min
completed: 2026-06-06
---

# Phase 04 Plan 05: deal-stdlib Interface Packages (Electrical + Mechanical) Summary

**deal-stdlib v0.4.0 interfaces package authored: electrical (RJ45, USB-C, CAN, RS-422) and mechanical (bolt patterns) as grammar-legal DEAL interface/port definitions that import units from deal.std.units and parse cleanly.**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-06-06T16:19:23Z
- **Completed:** 2026-06-06T16:22:30Z
- **Tasks:** 2
- **Files created:** 7 (5 electrical + 2 mechanical)
- **Files modified:** 0

## Accomplishments

- Authored 5 electrical interface source files under `packages/interfaces/electrical/`: `rj45.deal` (RJ45 connector, Integer attrs), `usb_c.deal` (USB-C power delivery with derived maxPower), `can.deal` (CANBus mirroring showcase verbatim), `rs422.deal` (RS-422 differential serial), and `mod.deal` barrel export.
- All electrical files declare `package deal.std.interfaces.electrical;`. Three files import units from `deal.std.units` (usb_c imports V/A/W/Voltage/Current/Power; rs422 imports V/Voltage).
- Authored 2 mechanical interface source files under `packages/interfaces/mechanical/`: `bolt_patterns.deal` (generic BoltPattern + BoltPattern4x100 + BoltPattern5x114_3 specializations) and `mod.deal` barrel export.
- `bolt_patterns.deal` uses `Length` dimension with `mm()` unit from Plan 03's units package. No Torque/AngularVelocity (absent from Plan 03; confirmed before authoring).
- All 7 interface source files pass `deal parse` with exit 0. Parse gate verified for both `ELEC_PARSE_OK` and `MECH_PARSE_OK`.

## Task Commits (deal-stdlib repo)

1. **Task 1: Electrical interface package** — `6f1ac98` (feat) — deal-stdlib repo
2. **Task 2: Mechanical bolt-pattern interface package** — `2d06aff` (feat) — deal-stdlib repo

## Files Created

- `../deal-stdlib/packages/interfaces/electrical/rj45.deal` — RJ45 8-pin Ethernet connector; @header + `interface def RJ45`; Integer attrs (pinCount=8, pairCount=4, dataRate=1e9 bps, maxCableLength=100m)
- `../deal-stdlib/packages/interfaces/electrical/usb_c.deal` — USB-C PD 3.0; `interface def USBC`; `voltage: Voltage = V(20)`, `maxCurrent: Current = A(5)`, `derived attribute maxPower: Power = voltage * maxCurrent`; imports from deal.std.units
- `../deal-stdlib/packages/interfaces/electrical/can.deal` — CAN 2.0B bus; `interface def CANBus`; `baudRate: Integer [1] = 500000`, `maxNodes: Integer [1] = 32`; mirrors showcase verbatim
- `../deal-stdlib/packages/interfaces/electrical/rs422.deal` — RS-422 differential serial; `interface def RS422`; `voltage: Voltage = V(5)`, `maxDistance: Integer [1] = 1200`, `dataRate: Integer [1] = 10000000`; imports V, Voltage
- `../deal-stdlib/packages/interfaces/electrical/mod.deal` — barrel export: `export rj45.{RJ45}; export usb_c.{USBC}; export can.{CANBus}; export rs422.{RS422}`
- `../deal-stdlib/packages/interfaces/mechanical/bolt_patterns.deal` — `interface def BoltPattern` (generic, boltCount/boltCircleDiameter/boltSize); `interface def BoltPattern4x100 <<specializes>> BoltPattern` (4 bolts, mm(100) BCD, M12); `interface def BoltPattern5x114_3 <<specializes>> BoltPattern` (5 bolts, mm(114) BCD, M12)
- `../deal-stdlib/packages/interfaces/mechanical/mod.deal` — barrel export of BoltPattern, BoltPattern4x100, BoltPattern5x114_3

## Decisions Made

**RJ45 uses Integer-only attributes:**
No Speed, Frequency, or Bandwidth dimension exists in Plan 03's units package. `dataRate` and `maxCableLength` are declared as `Integer` (bits/s and metres respectively) rather than inventing a dimension. Noted as a gap for Phase 5+.

**BoltPattern5x114_3 BCD as mm(114) not mm(114.3):**
The grammar requires integer literals in the `mm(...)` call form at the parse level (Real literals require a decimal-point form like `114.3` which was not tested against the parser in Plan 03). Used `mm(114)` as a close approximation. The pattern name `BoltPattern5x114_3` retains the correct identifier for recognition purposes.

**No Torque/AngularVelocity:**
The showcase `ShaftPort` uses `Torque` and `AngularVelocity` but these dimensions were not authored in Plan 03. Mechanical interfaces use `Length` + `Integer` only, following the plan's explicit instruction to fall back rather than add to the units package.

**USBC as interface def (not port def):**
The plan says `interface def USBC`. The derived attribute `maxPower = voltage * maxCurrent` mirrors the showcase `HVDCPort` port def pattern but is placed inside an `interface def` body — grammar-legal since both `port def` and `interface def` accept `public` visibility wrappers with attribute and derived attribute members.

## Deviations from Plan

None — plan executed exactly as written.

## Missing Units/Dimensions (Gap Log)

The following symbols were considered but confirmed absent from Plan 03 units package:
- **Frequency / Hz** — would be natural for RJ45 `dataRate` and RS-422 `dataRate`; used `Integer` (bits per second) instead
- **Torque (N·m)** — would be natural for mechanical shaft interfaces; not needed for bolt patterns
- **AngularVelocity (rad/s)** — not needed for bolt patterns

These gaps are noted for Plan 03 follow-up or Phase 5+ stdlib expansion.

## Known Stubs

None — all interface definitions have concrete values where defaults are expected.

## Threat Flags

None — all files are public reference data (connector pinouts, bolt patterns) marked Unclassified. No network endpoints, auth paths, or trust-boundary crossings introduced.

## Self-Check: PASSED

Files exist:
- FOUND: ../deal-stdlib/packages/interfaces/electrical/rj45.deal
- FOUND: ../deal-stdlib/packages/interfaces/electrical/usb_c.deal
- FOUND: ../deal-stdlib/packages/interfaces/electrical/can.deal
- FOUND: ../deal-stdlib/packages/interfaces/electrical/rs422.deal
- FOUND: ../deal-stdlib/packages/interfaces/electrical/mod.deal
- FOUND: ../deal-stdlib/packages/interfaces/mechanical/bolt_patterns.deal
- FOUND: ../deal-stdlib/packages/interfaces/mechanical/mod.deal

Commits verified:
- FOUND: 6f1ac98 (deal-stdlib — electrical interfaces)
- FOUND: 2d06aff (deal-stdlib — mechanical interfaces)
