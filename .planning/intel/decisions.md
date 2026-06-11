# DEAL Decisions Intel

Source ADR (consolidated): `/Users/dunnock/projects/deal-lang/spec/grammar/DESIGN-DECISIONS.md`
Precedence: 0 (manifest override)
Status: 64 LOCKED + 1 CAPTURED across 8 categories
Session: 2026-05-16 (3 sessions)

Every entry below is preserved individually; the consolidated document treats each `XX-N` code as an independently-locked decision.

---

## Foundational Architecture

### FA-1: DEAL is a systems engineering compiler [LOCKED]

- **Source:** `/Users/dunnock/projects/deal-lang/spec/grammar/DESIGN-DECISIONS.md`
- **Status:** LOCKED
- **Scope:** Compiler architecture, intermediate representation, codegen pipeline
- **Decision:** DEAL is a systems engineering compiler infrastructure — a text-first authoring language with its own intermediate representation (IR) that transpiles to multiple target formats. Key properties: (1) DEAL IR is the kernel, not SysML v2 JSON; (2) SysML v2 JSON is one export target among several; (3) the IR carries everything (structure, agent metadata, simulation bindings); (4) import and export are separate paths; (5) the DEAL source file is the canonical artifact and single source of truth.
- **Pipeline:** DEAL Source → AST → DEAL IR (kernel) → multiple codegen backends (SysML v2 JSON, ReqIF XML, FMI/FMU, ICD packages, Documents).

### FA-2: Object-oriented library system is first-class [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Language features, standard library, package system
- **Decision:** Reusable, composable, versionable libraries as a core language feature. Standard library includes physical interfaces, protocols, units, and domain patterns (e.g., `deal install @deal-stdlib/mil-std-1553`).

### FA-3: Dual-purpose design [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Audience, design priorities
- **Decision:** DEAL serves defense/aerospace systems engineers and software engineers building integrations equally. When tradeoffs arise, they are documented.

### FA-4: Self-explanatory syntax principle [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Syntax design, AI-readability
- **Decision:** Every construct must be understandable by an AI agent or human reading the file cold — without external documentation or language training.

### FA-5: Simulation integration is first-class [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Simulation, runtime I/O
- **Decision:** Models serve as simulation inputs and consume simulation outputs. JSON as universal I/O format. `deal_sim` Python SDK. `deal simulate` CLI.

---

## Naming

### NC-1: DEAL — Digital Engineering Authoring Language [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Language identity
- **Decision:**
  - Language: DEAL — Digital Engineering Authoring Language
  - CLI: `deal`
  - Extensions: `.deal` (definitions) / `.dealx` (compositions)
  - Organization: deal-lang (GitHub)
  - Website: deal-lang.org

---

## Language Model Alignment

### LM-1: TypeScript as primary language feel [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Surface syntax conventions
- **Decision:** DEAL follows TypeScript conventions wherever applicable. Deviations only for systems engineering constructs with no TypeScript analog.

### LM-2: Doc comments use JSDoc style [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Documentation syntax
- **Decision:** Use `/** ... */` with `@tag` directives (e.g., `@see REQ_SYS_001`). Doc comments precede declarations.

### LM-3: String literals — TypeScript model [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** String literal syntax
- **Decision:** Double quotes for strings; backticks for multi-line and template strings (`${expr}` interpolation). Single quotes accepted as alias; `deal fmt` normalizes to double quotes.

---

## File Structure

### FS-1: File anatomy [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** .deal and .dealx file top-level structure
- **Decision:** Optional `@header { ... }` block → `package` declaration → zero or more `import` statements → model content (definitions / exports for `.deal`, compositions / inline defs for `.dealx`).

### FS-2: @header block with CM fields [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Configuration management metadata
- **Decision:** `@header` is a brace block of `key: value` lines. Fields: `path`, `schema`, `created`, `modified`, `reviewed`, `hash`, `status` (one of draft|review|baseline|superseded), `baseline`, `marking`. Created/modified/reviewed lines include timestamp + `by "Name <email>"` + optional `via tool-id`.

### FS-3: Attribution model [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Authorship, tool attribution
- **Decision:** `by` always identifies a human (from `deal config`). `via` optionally identifies the tool (`deal:fmt`, `deal:import`, `claude:claude-code`). Identity is git-style (`deal config --global user.name`/`user.email`). `deal fmt` refuses to run without configured identity. Human is accountable; AI/tool is informational.

### FS-4: System-wide index [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Project-wide symbol resolution
- **Decision:** No per-file index. Project-wide index generated by `deal index`, stored at `.deal/index.json`, gitignored. Queried by `deal query`, LSP, and MCP server.

---

## Project Structure

### PS-1: deal.toml project manifest [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Project manifest format
- **Decision:** Root TOML manifest with sections: `[project]` (name, version, schema, marking), `[workspace]` (packages glob), `[workspace.aliases]`, `[dependencies]`, `[simulations]` (registry, cache_dir), `[build.targets]` (sysml-v2, reqif, docs entries).

### PS-2: Package declarations with dot paths [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Package syntax
- **Decision:** `package sedan_project.vehicle;` — dot-separated identifiers.

### PS-3: Barrel exports via index.deal [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Module export convention
- **Decision:** Explicit selective exports follow TypeScript `index.ts` pattern: `export electrical.{HVDCPort, CANBus};`. Barrel files are named `index.deal` to match LM-1's TypeScript feel.

### PS-4: Five import forms [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Module import syntax
- **Decision:** Five forms: `import deal.std.units.{kg, m, s};` (external), `import tracking.{Radar};` (workspace alias), `import .local_module.{HelperType};` (relative same package), `import ..sibling_package.{SharedInterface};` (relative parent), `import interfaces.*;` (barrel glob), and `import interfaces as intf;` (module reference + alias). (The text lists five forms plus the aliased form.)

### PS-5: Workspace aliases — pnpm-style [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Multi-package workspace resolution
- **Decision:** `[workspace.aliases]` table maps alias names to package paths (e.g., `tracking = "packages/subsystems/tracking"`).

### PS-6: Unit literals — function form from libraries [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Numeric unit literals
- **Decision:** Units are functions imported from libraries: `import deal.std.units.{kg, m, s, V, A, kW}; attribute mass : Mass = kg(1500);`.

### PS-7: Build targets in deal.toml [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Codegen target configuration
- **Decision:** `[build.targets]` table maps target names to `{ format, output }` records (e.g., `sysml-v2 = { format = "json", output = "build/sysml-v2/" }`).

### PS-8: Standard directory layout [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Project on-disk structure
- **Decision:** Standard tree: `deal.toml`, `.deal/` (generated, gitignored, with `index.json` + `simulations/` cache), `model/` (compositions `.dealx`), `packages/` (definitions `.deal` — vehicle, interfaces, requirements, use-cases), `simulations/` (with `deal.sims.toml`), `test/data/`, `docs/`.

### PS-9: deal config for user identity [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** User identity for attribution
- **Decision:** Git-style identity configuration: `deal config --global user.name "..."` and `user.email "..."`.

### PS-10: .deal/ generated directory [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Build artifact location
- **Decision:** `.deal/` is gitignored. Contains index, parser cache, and simulation I/O cache.

---

## Syntax — Definitions

### SD-1: Element keywords match SysML v2 vocabulary [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Definition keywords
- **Decision:** Element-`def` keywords: `part def`, `port def`, `action def`, `state def`, `requirement def`, `constraint def`, `attribute def`, `item def`, `interface def`, `connection def`, `flow def`, `allocation def`.

### SD-2: Typing uses colon [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Type annotation syntax
- **Decision:** `part engine : Engine;` / `attribute mass : Mass;` — colon between name and type.

### SD-3: Double angle bracket delimiters [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Operator delimiters
- **Decision:** Use `<<operator>>` for structural relationships: `<<specializes>>`, `<<redefines>>`, `<<subsets>>`, `<<satisfies>>`, `<<allocated to>>`, `<<derives from>>`. Zero collision with any operator; ASCII approximation of UML guillemets.

### SD-4: Two-tier operator system [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Operator tiering
- **Decision:** Tier 1 = declaration-level (no prefix) — used in `part def Sedan <<specializes>> Vehicle { }`. Tier 2 = annotation-level (with `@category:` prefix) — used in `@trace:<<satisfies>> REQ_SYS_001`, `@connection:<<connects>> fuelPort to engine.intake`, `@simulation:<<computes>> thermalProfile { ... }`.

### SD-5: SysML v2 symbolic operators as import aliases [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** SysML v2 interop
- **Decision:** SysML v2 symbolic operators are import aliases: `:>` → `<<specializes>>`, `:>>` → `<<redefines>>`, etc. `deal import` reads SysML v2; `deal fmt` normalizes to DEAL canonical form.

### SD-6: Relationship categories [CAPTURED]

- **Source:** `DESIGN-DECISIONS.md`
- **Status:** CAPTURED (not LOCKED)
- **Scope:** Annotation category namespaces
- **Decision:** Defined categories: `@trace:`, `@connection:`, `@behavioral:`, `@flow:`, `@state:`, `@temporal:`, `@requirement:`, `@simulation:`, `@document:`. Full enumeration left deferred.

### SD-7: Modifiers are bare keywords [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Element modifier syntax
- **Decision:** Modifiers like `abstract`, `derived`, `readonly`, `ordered` are bare keywords prefixing declarations.

### SD-8: Direction keywords are bare [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Port direction syntax
- **Decision:** `in`, `out`, `inout` are bare keyword prefixes on port declarations.

### SD-9: Visibility as scope wrappers [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Visibility syntax
- **Decision:** `public ( ... )`, `protected ( ... )`, `private ( ... )` wrap member declarations inside a definition body. Default visibility when no wrapper is present is deferred.

### SD-10: Semicolons [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Statement termination
- **Decision:** Required on single-line declarations. Optional after block-closing braces.

### SD-11: Comments [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Comment syntax
- **Decision:** `// single-line`, `/* block comment */`, `/** JSDoc doc comment */`.

### SD-12: String literals [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** String literal syntax
- **Decision:** Double quotes for strings. Backticks for multi-line and templates. Single quotes accepted; `deal fmt` normalizes to double. (Consistent with LM-3.)

### SD-13: Unit literals [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Unit literal syntax
- **Decision:** Function form from libraries: `kg(1500)`, `V(800)`, `km(200) / hr(1)`. (Consistent with PS-6.)

### SD-14: Multiplicity [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Cardinality syntax
- **Decision:** Square-bracket multiplicities: `[4]` exactly 4, `[1..5]` range, `[*]` zero or more, `[1..*]` one or more, `[0..1]` optional. Appears after the type annotation.

### SD-15: Annotation value syntax [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Annotation payload syntax
- **Decision:** Inline single-value form `@confidence: 0.85` / `@assumes: "..."`. Braces for multi-field payloads like `@simulation:<<computes>> thermalProfile { equation: "...", tool: "python", entry: "..." }`.

### SD-16: Doc comments + structured annotations [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Documentation vs metadata split
- **Decision:** `/** */` for narrative documentation. `@` annotations for queryable metadata.

### SD-17: File extensions [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** File-type discrimination
- **Decision:** `.deal` for definition files (components, interfaces, types). `.dealx` for composition files (systems, subsystems, wiring). `.dealx` can contain inline definitions for one-off types. `deal fmt` warns if an inline definition in `.dealx` is referenced by multiple compositions. `.deal` files never import from `.dealx` — unidirectional dependency. Parser uses extension to select grammar mode.

### SD-18: Needs, requirements, and use cases as definition keywords [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Requirement modeling keywords
- **Decision:** `need def NEED_RANGE { ... }`, `requirement def REQ_SYS_001 { ... }`, `use case def LongDistanceTrip { ... }`. Definitions live in `.deal` files; allocations and traceability live in `.dealx` files.

### SD-19: Definitions and allocations are separated [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** WHAT vs HOW split
- **Decision:** `packages/` (.deal) holds WHAT: requirements/needs, use-cases, vehicle parts. `model/` (.dealx) holds HOW: architecture, traceability, variants.

### SD-20: Requirements declare verification contracts [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Requirements + verification
- **Decision:** `requirement def` body contains a `verification { ... }` block with fields: `accepts: [methods]` (satisfying verification methods), `rejects: [methods]` (explicitly insufficient), `threshold: attribute` (comparison attribute), `operator: ">="|"<="|"=="` (comparison direction), `conditions:` (environmental conditions requiring separate evidence). `deal check` enforces method type-checking against accepts/rejects.

---

## Syntax — Compositions

### CS-1: Dual syntax model [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Definition vs composition file syntax
- **Decision:** Definition files (`.deal`) use bare keyword definitions (`part def`, `port def`, `connection def`, `flow def`). Composition files (`.dealx`) use `[<tag>]` syntax for systems, subsystems, component instances, connect, expose blocks.

### CS-2: [< >] composition tags [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Composition syntax delimiters
- **Decision:** Composition blocks use JSX-like tag syntax: `[<system EVPlatform>] ... [</system>]`, `[<subsystem EnergyStorage>] ... [</subsystem>]`, self-closing instances `[<BatteryPack as="battery" voltage={V(800)} />]`, attributes `[<connect from="a" to="b" via={Cable {...}} carrying={Power {...}} />]`, exposed ports `[<expose battery.hvOut as="hvPowerOut" />]`.

### CS-3: Component instances use `as` [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Instance naming
- **Decision:** Instance binding uses an `as="name"` attribute on the component tag.

### CS-4: Physical/logical connection separation [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Connect block semantics
- **Decision:** `[<connect>]` blocks separate `via=` (physical connection: cable, harness, hose, typed against `connection def`) from `carrying=` (logical flow: power, data, coolant, typed against `flow def`).

### CS-5: connection def / flow def keywords [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Connection vs flow definition keywords
- **Decision:** `connection def HVDCCable { ... }` for physical medium. `flow def PowerDelivery { ... }` for logical content.

### CS-6: expose keyword [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Port exposure
- **Decision:** `[<expose battery.hvOut as="hvPowerOut" />]` exposes an internal port as a subsystem-level port.

### CS-7: Hierarchical nesting [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Composition hierarchy
- **Decision:** System > subsystem > subsystem. Each level has internal wiring and exposed interfaces.

### CS-8: Prop validation via multiplicity [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Component prop validation
- **Decision:** Component definitions declare props with multiplicity. `deal check` validates that compositions satisfy required props (`[1]`), respect optionals (`[0..1]`), and meet collection minimums (`[1..*]`).

### CS-9: Traceability composition block [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Traceability containers
- **Decision:** `[<traceability EVPlatformTraces>]` encloses `[<allocate>]`, `[<satisfy>] => { ... }`, and `[<validate>]` child blocks.

### CS-10: Satisfy block with executable criteria and typed returns [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Requirement satisfaction syntax
- **Decision:** `[<satisfy requirement="..." by="..." method="...">] => { returnFields }` body contains `criteria { boolean exprs }`, `evidence simulation { source, binding, maps { src -> dst } }`, `compute { derived = ... }`, optional `gap { risk, mitigation }`. `method` is type-checked against the requirement's `verification.accepts`. `=> { ... }` declares typed return values (TypeScript arrow syntax). `deal check --verify` evaluates all criteria and reports PASS/FAIL/PARTIAL.

### CS-11: Validate block [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Validation evidence
- **Decision:** `[<validate requirement="..." by="...">]` body has `scenario:`, `status:` (e.g., "passed"), `evidence:` text fields.

### CS-12: Allocate block [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Allocation relationships
- **Decision:** `[<allocate from="..." to="..." relationship=<<derives>> />]` declares directed allocation with a typed relationship.

### CS-13: Satisfy returns populated via evidence maps [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Evidence binding
- **Decision:** `evidence simulation { source: "...", binding: "...", maps { totalRange -> actualRange } }` — the `maps` block connects simulation/test output fields to return value names.

### CS-14: Satisfy returns derived via compute blocks [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Derived return values
- **Decision:** `compute { margin = actualRange - REQ_SYS_001.minRange; marginPercent = margin / REQ_SYS_001.minRange * 100; }` derives additional return values from primary returns and requirement attributes.

### CS-15: Return values referenceable as TraceName.ReqID.fieldName [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Cross-file reference syntax
- **Decision:** Satisfy return values are addressable anywhere in the model via `TraceName.ReqID.fieldName` — e.g., `EVPlatformTraces.REQ_SYS_003.actualMass` or `${EVPlatformTraces.REQ_SYS_001.marginPercent}`.

### CS-16: Verification method type-checking [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Verification method enforcement
- **Decision:** If a requirement declares `accepts: [test]` and a satisfy block specifies `method="simulation"`, `deal check` reports an error. The verification method must match the requirement's accepted list.

---

## Simulation Integration

### SIM-1: JSON as universal simulation I/O [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Simulation I/O contract
- **Decision:** `DEAL model → input.json → Simulation → output.json → DEAL model`. JSON is the wire format for all simulation interaction.

### SIM-2: deal_sim Python SDK [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Python simulation SDK
- **Decision:** `from deal_sim import DealSimulation` — subclass declares typed `inputs = {...}` / `outputs = {...}` dicts (with `type` and `unit`) and a `run(self, inputs) -> dict` method. `if __name__ == "__main__": MySim.cli()` activates CLI entry point.

### SIM-3: Simulation registry — deal.sims.toml [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Simulation discovery
- **Decision:** `deal.sims.toml` registers simulations: `[simulations.<name>]` with fields `tool`, `entry`, `class`, `binds_to` (model path like `packages/vehicle/battery.deal::BatteryPack`), `annotation` (the `@simulation:` annotation it implements), and arrays of `inputs`/`outputs` `{ model_path, param, unit }` records.

### SIM-4: Simulation CLI commands [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Simulation CLI surface
- **Decision:** Commands: `deal simulate <name>` (run specific), `deal simulate --all`, `deal simulate --stale` (run only stale), `deal check --simulations` (validate bindings), `deal check --verify` (evaluate criteria from cache), `deal check --verify --run-sims` (re-run stale then evaluate), `deal evidence capture` (snapshot results), `deal evidence baseline v2.1.0` (tag with baseline).

### SIM-5: Three-level verification [LOCKED]

- **Source:** `DESIGN-DECISIONS.md`
- **Scope:** Verification pipeline
- **Decision:** Level 1 — Structural completeness: every need traces to requirement; every requirement has satisfaction; every satisfaction has evidence. Level 2 — Criteria evaluation: evaluates boolean criteria from `[<satisfy>]` blocks; compares model values against requirement thresholds; returns PASS/FAIL/PARTIAL with actual values and margins. Level 3 — Evidence freshness: detects stale simulation results; validates test data file paths exist; recommends re-runs.

---

## Decisions Deferred (per ADR section)

The ADR explicitly defers these — recorded here so downstream planning does not assume they are locked:

- Expression syntax details (arithmetic, comparison, logical)
- Path expression syntax
- Full relationship category enumeration
- MQL query language design
- Codegen backend API
- Default visibility when no wrapper
- Constraint expression syntax
- `@document:` category design
- `[<document>]` composition block for report generation

---

*Total: 65 decisions extracted (64 LOCKED + 1 CAPTURED). Source: 2026-05-16 consolidated design log.*
