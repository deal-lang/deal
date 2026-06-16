# deal-lang/deal

Core DEAL compiler. The language engine every tool wraps — CLI, LSP, the VS Code
extension, the future desktop editor, and the code-generation backends.

The compiler **core** is written in [Zig](https://ziglang.org/) and built as a
static library (`libdeal.a`) with a narrow C ABI. The **integration shell** —
the CLI and the LSP server — is written in Rust and links the core through that
ABI.

```
Zig compiler core (libdeal.a)        Rust integration shell
  lexer → parser → AST                 deal   (CLI driver)
  semantic analysis + dimensions       deal-lsp (language server)
  DEAL IR + lowering                   deal-ffi (safe FFI wrapper)
  SysML v2 / ReqIF code generation
            └──────────── C ABI: deal_parse() / deal_check_with_stdlib() / … ──┘
```

Full language documentation, guides, and the language reference live at
**[deal-lang.org](https://deal-lang.org)**.

## Status

The parser, semantic analyzer, formatter, IR, SysML v2 / ReqIF backends, the
`calc` / `constraint` surface (SD-21/22/23), project/dependency tooling, and the
simulation-evidence pipeline are implemented and gated by the test suite.

**Stage 2 — the behavioral surface (BH-1..BH-7) is complete** end to end: actions
and state machines (pins, succession, `decide`/`par`, loops, send/accept/assign,
perform, item flow, bind, transitions, entry/do/exit) parse, resolve, format
idempotently, lower to IR v0.1, and emit OMG-schema-valid SysML v2. The showcase
(`behaviors.deal`, `charging-states.deal`) round-trips through the whole pipeline.
See [`CHANGELOG.md`](./CHANGELOG.md). One deferral remains: behavioral guards /
values are carried as text and not yet emitted as structured SysML `Expression`
trees (Stage-3 candidate, `spec/ir/v0.1/FUTURE-structured-expressions.md`).

Not yet started: the editor-first platform (Phase 6) — the Tauri desktop editor,
import pipelines (`deal import`), and documentation generation
(`deal build --target docs`).

## What the CLI does today

```
deal parse     <paths>                 tokenize + parse .deal/.dealx → AST JSON
deal check     <paths> [--verify]      semantic + dimensional analysis;
                       [--simulations]   --verify evaluates requirement criteria
                       [--run-sims]      against captured evidence
deal fmt       <paths> [--check]       format in place (--check / --stdout)
                       [--stdout]
deal build     --target sysml-v2|reqif generate SysML v2 JSON or a ReqIF archive
                       [--validate]      (--validate runs offline schema checks)
                       [--output <p>]
deal init      <name>                  scaffold a new project
deal install                           resolve deal.toml deps → deal.lock
deal simulate  [names] [--all]         run simulations from deal.sims.toml
                        [--stale]
deal evidence  <subcommand>            capture / manage verification evidence
```

## Build from source

Requires **Zig 0.16.0** and a **Rust** toolchain (stable) on your `PATH`.

```bash
git clone https://github.com/deal-lang/deal.git
cd deal
git submodule update --init --recursive   # fetches the spec/ submodule + showcase
cargo build --release                      # build.rs runs `zig build` automatically
```

The compiled binary is `target/release/deal`. Cargo's FFI build script shells out
to `zig build` to produce `libdeal.a`, so `cargo build` builds the whole pipeline
in one step — if it errors about `zig build`, confirm `zig version` reports
`0.16.0`.

### Submodule note

The grammar spec and showcase corpus live in
[`deal-lang/spec`](https://github.com/deal-lang/spec), mounted as a submodule at
`./spec` and symlinked into the test suite at `tests/showcase`. After any fresh
clone or `git worktree add`, run `git submodule update --init --recursive` — a
number of tests resolve through that mount and fail with `error.FileNotFound`
without it.

## Repository layout

```text
deal/
├── build.zig            Zig build (emits libdeal.a)
├── include/deal.h       C ABI header
├── src/                 Zig core — lexer, parser.zig, parser_dealx.zig,
│                        sema.zig, lowering.zig, fmt.zig, json.zig, lib.zig (ABI)
├── deal-ffi/            Rust ↔ Zig FFI surface (build.rs links libdeal.a)
├── cli/                 Rust CLI (`deal`) — subcommands above
├── lsp/                 Rust language server (`deal-lsp`)
├── tests/               Zig snapshot + malformed-input tests; showcase symlink
└── spec/ (submodule)    grammar + showcase corpus
```

## Testing

```bash
zig build test            # Zig unit + snapshot tests
zig build phase-2-gate    # phase exit gate (inherits earlier phases)
cargo test --workspace    # Rust CLI + FFI integration tests
```

## License

Apache-2.0. See [`LICENSE`](./LICENSE).
