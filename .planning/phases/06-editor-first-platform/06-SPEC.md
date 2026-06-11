---
phase: 6
slug: editor-first-platform
status: approved-design
created: 2026-06-08
approved: 2026-06-08
source: brainstorming-session
---

# Phase 6A — Editor-First Platform Spec

## Purpose

Split the current broad Phase 6 "Application" objective into a dedicated editor milestone before import pipelines, document generation, and stdlib expansion.

The milestone delivers DEAL's local-first desktop authoring experience: a fast, clean, impressive workbench where engineers edit canonical text, interact with generated model views, and see live model state from both the embedded editor and external IDEs.

## Scope

### In Scope

- Tauri v2 desktop application with a React + Vite frontend.
- `deal dev` local live model daemon used by Tauri and external IDE/LSP workflows.
- Full-file CodeMirror editor with LSP intelligence.
- Lightweight span-scoped CodeMirror quick editor.
- Rust-to-WASM WebGL2 diagram renderer with WebGPU deferred behind explicit parity and performance gates.
- Three-column workbench MVP: navigation, primary surfaces, inspector/status.
- Modular panel architecture so future docking or alternate shells do not rewrite panel internals.
- `.dealview` authoring and validation: theme files, per-view sidecars, layout deltas, include/exclude membership, and `deal check --views` integration.
- Canvas-handle semantic editing for structure and traceability views.
- Classification and portion-marking support per `ADR-portion-marking-classification.md`.
- Git-aware status and safeguards: branch display, dirty indicators, diff visibility, and last app-originated patch revert.
- CLI-backed check, simulation, and export orchestration, with the UI acting as viewer/editor rather than reimplementing processors.
- Signed/notarized manually distributed binaries for demo use on supported platforms.

### Out of Scope

- SysML v2 and ReqIF import pipelines.
- Generated document backend.
- `deal-stdlib` expansion.
- Cloud/SaaS hosting or remote compilation.
- Real-time multi-user collaboration.
- Auto-update channels.
- Full accessibility conformance as an MVP gate.
- Full VS Code-style docking, detachable windows, or arbitrary panel layout customization.

## Exit Criteria

A user can:

1. Open a local DEAL project in a signed desktop binary.
2. Edit `.deal`, `.dealx`, and `.dealview` files in the app.
3. Edit the same project from an external IDE and see the desktop UI update from unsaved LSP changes.
4. Use diagram canvas handles to make structure and traceability edits that patch canonical source.
5. Maintain diagram membership/layout as versioned `.dealview` artifacts.
6. See classification banners, portion marks, downgrade diagnostics, Git status, verification status, and simulation status in the workbench.
7. Run or observe DEAL CLI-backed check/simulate/export actions without the UI owning processor logic.

## Architecture

### Zig Compiler Core

The Zig core remains the language authority. It owns parse, format, diagnostics, and IR generation through the existing C ABI. The desktop editor, `deal dev`, and renderer do not duplicate compiler semantics.

### Rust Model Service

A reusable Rust model-service crate powers both `deal dev` and the Tauri backend. It owns:

- Project loading and workspace graph state.
- File watching and external edit ingestion.
- Debounce/coalescing and latest-wins parse scheduling.
- IR diffing and sequence numbering.
- Snapshot recovery.
- Git status and diff metadata.
- CLI command orchestration.
- Span-anchored source patch application.
- Classification and view metadata carried from compiler/model outputs to clients.

### `deal dev` Daemon

`deal dev` is a local-only live model daemon. It exposes authenticated JSON-RPC/WebSocket endpoints for:

- `deal/ir` snapshots.
- `deal/irDidUpdate`-style deltas.
- Diagnostics and timing metadata.
- Command status for check/simulate/export orchestration.
- Git status and dirty-file metadata.

It also serves a lightweight local diagnostics UI with connected clients, current branch, dirty files, last IR sequence, `clean | held` state, parse/check timing, and recent events.

The daemon binds local-only by default and uses an ephemeral local token or OS-owned socket/pipe credential for client authentication.

### Tauri Desktop App

The Tauri app starts, embeds, or connects to the same model service used by `deal dev`. The frontend subscribes to model updates and sends editor/canvas commands. It does not own model truth.

Tauri is responsible for:

- Native desktop packaging.
- Local project open/save UX.
- Native file dialogs.
- Secure bridge from React to the model service.
- Manual signed/notarized binary distribution.

### Frontend Stack

Use React + Vite instead of Next.js. Tauri production builds consume static frontend assets, and the editor does not need Next.js routing, SSR conventions, or server-side features.

Primary frontend modules:

- Workbench shell.
- Panel registry and command bus.
- Shared selection context.
- CodeMirror editor surfaces.
- Renderer canvas host and overlays.
- Inspector and quick edit panels.
- Git/diff/status panels.
- Classification/status banners.
- CLI action/result views.

### Renderer

Use a Rust-to-WASM renderer kernel rather than the Zig-to-WASM direction previously proposed in `ADR-graphical-view-projection-and-sync.md`.

Rationale:

- GPU batching, text rendering, viewport culling, and data layout dominate renderer performance more than Zig-vs-Rust CPU differences.
- Rust has the more mature browser/WASM tooling path and fits the Tauri/model-service architecture.
- Renderer types can share Rust IR/delta/view model structures with the service and tests.
- Zig remains focused on compiler/parser/formatter ownership.

Renderer responsibilities:

- Consume IR and `.dealview` deltas.
- Maintain layout, hit-test index, viewport/camera state, culling, and draw-list generation.
- Implement SysML v2 graphical notation draw templates.
- Use WebGL2 as the required backend.
- Keep WebGPU as a criteria-gated future backend after golden-image parity and benchmark wins.
- Prototype and verify crisp text rendering early, using a glyph atlas/SDF or a measured equivalent.

## Data And Sync

### Update Model

The model service publishes ordered workspace updates. Each parse/lower result has:

- Workspace epoch.
- Buffer version.
- Monotonic sequence number.
- Status: `clean` or `held`.
- Diagnostics.
- Timing metadata.

Clients connect by requesting a `deal/ir` snapshot, then consume deltas. Deltas include:

- Added, removed, and changed nodes.
- Added, removed, and changed edges.
- Affected source spans.
- Marking rollups.
- View membership effects.
- Dirty-file metadata.

Sequence gaps trigger snapshot recovery.

### Renderability Contract

The service implements the anti-glitch contract from `ADR-graphical-view-projection-and-sync.md`:

- Edits are debounced and parsed off-thread.
- Stale parse results are dropped when newer edits arrive.
- Semantic diagnostics can still render when usable IR exists.
- Syntax/lowering failures produce `held` and preserve the last good renderable IR.
- The diagram never blanks or renders partial garbage due to an incomplete edit.

### External IDE Sync

External IDEs update the app immediately through the DEAL LSP path. IDE `didChange` notifications carry unsaved edits to the shared model service, which emits IR deltas to Tauri clients. Filesystem watch is the universal save-based fallback for tools without LSP participation.

### Edit Model

- Full-file CodeMirror edits update in-memory buffers and flow through the same debounce/delta path.
- Quick editor edits are span-scoped and lightweight. Apply immediately patches canonical source, runs format/check, and records an app-originated patch for sidebar revert.
- Canvas semantic edits use viewpoint-specific handles and also create span-anchored source patches.
- Structure views can create/edit parts and connects.
- Traceability views can create/edit satisfy, verify, and allocate relationships.
- Layout-only edits write debounced `.dealview` deltas and do not reparse `.deal` or `.dealx`.
- New semantic content created through a view lands in that view's bound source file, matching the ADR edit-target rule.

### Persistence

- Canonical project source: `.deal`, `.dealx`, `.dealview`, `deal.toml`.
- Versioned diagram/view state: `.dealview` files.
- User-local UI preferences: sidebar sizes, collapsed panels, active tab, theme, recent projects.
- Local cache: render caches, glyph atlases, CLI output cache, model-service temp state.
- Git: observed for branch, dirty files, diffs, and targeted revert; the editor does not auto-commit.

## Workbench UX

### Default Layout

The MVP uses the current three-column workbench:

- Left: segmented navigation for Files, Structure, and Views.
- Center: primary surfaces for Diagram, Editor, Requirements, Traceability, Verification/Simulation, and Diff.
- Right: inspector, lightweight quick editor, selected-element diagnostics, Git/revert actions, and contextual status.
- Top/bottom status: branch, dirty state, model service state, LSP state, classification banner, current file/view, and check/simulation state.

The shell is fixed for MVP, but panel internals are registered modules with stable inputs and commands so a later modular/dockable shell can replace the layout without rewriting panels.

### Editors

Full editor:

- CodeMirror with LSP diagnostics, completion, hover, and format hooks.
- Full-file context.
- Primary text authoring surface.

Quick editor:

- CodeMirror with syntax highlighting, bracket matching, indentation, selection, basic keymaps, and small undo history.
- Span-scoped document.
- No separate LSP client per instance.
- Validation/format/check delegated through the model service.
- One mounted instance reused across selections and lazy-mounted when the right panel is visible.

### Semantic Canvas Editing

Canvas handles are the primary semantic-editing pattern. Handles appear only when valid for the active viewpoint and current selection.

Rules:

- Structure affordances create parts and connects.
- Traceability affordances create satisfy, verify, and allocate relationships.
- Layout gestures write `.dealview`.
- Semantic gestures patch `.deal` or `.dealx`.
- Irrelevant actions are hidden to prevent menu bloat and cross-view mistakes.
- The right inspector remains available for details, advanced fields, and revert, but the main modeling motion happens on the canvas.

### CLI-Orchestrated Status

The UI may launch and observe DEAL CLI actions, but compiler, check, simulation, build, and export behavior remain CLI/model-service-owned.

The editor displays:

- `deal check` diagnostics.
- `deal check --views` diagnostics.
- `deal simulate` and `deal check --verify` status/results.
- Export/build command progress and result summaries.

## `.dealview` Requirements

The milestone includes `.dealview` authoring and validation:

- Workspace theme file.
- One file per view for instance layout and membership.
- Layout deltas for x/y/w/h, collapsed state, and edge waypoints.
- Include/exclude membership blocks.
- Orphaned rule diagnostics.
- No-fabrication validation: view files can reference/style existing elements but cannot declare model elements.
- `deal check --views` integration in CLI and UI.
- Diagram layout edits write versioned `.dealview`, not user-local preferences.

## Classification Requirements

Classification support follows `ADR-portion-marking-classification.md`.

Editor requirements:

- Display derived workspace/file/view high-water markings from IR.
- Highlight `@(...)` portion marks in full and quick CodeMirror editors.
- Propagate markings into diagram top/bottom banners and selected-element inspector.
- Surface `marking-policy` mode: `off`, `advisory`, `strict`.
- Show strict-policy diagnostics for downgrade leaks and missing inherited top-level marks.
- Show view/expose downgrade diagnostics in diagram and traceability surfaces.
- Treat `declassify` as an explicit audited construct, not a normal quick-edit shortcut.
- Do not derive classification state from frontend string parsing; use compiler/model-service data.

## Git Safeguards

The editor includes local Git-aware guardrails:

- Current branch display.
- Dirty file indicators.
- Diff panel for source and `.dealview` changes.
- Revert for the last app-originated quick editor or canvas semantic patch.
- Clear distinction between user-authored external changes and app-originated patches.

The editor does not auto-commit, auto-push, or manage PRs in this milestone.

## Packaging

The milestone includes both local developer workflow and demo-ready binaries:

- Dev workflow for local Tauri/React/Vite iteration.
- Signed/notarized production-style binaries for configured release platforms.
- macOS artifacts are signed and notarized.
- Windows artifacts are signed.
- Linux artifacts are packaged with release metadata, checksums, and signing where the selected package format supports it.
- Manual distribution only.
- No auto-update channel.

Credential prerequisites for signing/notarization are release-gate inputs. A configured platform's release gate is blocked until its credentials and signing path are available.

## Accessibility Posture

Full accessibility conformance is not an MVP acceptance gate. The architecture must avoid blocking later accessibility work:

- Use semantic HTML for chrome, trees, tables, tabs, forms, and inspector panels.
- Maintain a command model so actions are not only mouse gestures.
- Keep selection state independent of pointer events.
- Expose selected diagram element metadata to DOM/inspector surfaces.
- Preserve high-contrast classification banners.
- Do not encode classification solely by color.

Deferred accessibility work includes full keyboard diagram editing, screen-reader diagram traversal, and WCAG conformance testing.

## Quality Gates

Required verification:

- TypeScript typecheck and lint before frontend build.
- Rust tests for the model service, source patching, IR delta sequencing, Git status, and CLI orchestration.
- Existing Zig/compiler gates for parse/format/check.
- WASM renderer golden-image snapshots.
- Renderer hit-test fixtures.
- Renderer layout stability tests.
- WebGL2 nonblank canvas checks.
- Pan/zoom frame-time benchmark.
- 10k-element stress benchmark.
- Sync tests for external LSP unsaved edits, filesystem fallback, syntax-break hold-last-good, and sequence-gap snapshot recovery.
- `.dealview` tests for include/exclude resolution, orphaned rules, layout delta persistence, and no-fabrication diagnostics.
- Classification tests for portion highlighting, derived high-water banners, strict/advisory modes, view downgrade diagnostics, and declassify display.
- Packaging tests for signed/notarized manual artifacts with credentials documented.

## Roadmap Impact

This spec supersedes the current monolithic Phase 6 application shape for planning purposes.

Recommended split:

- **Phase 6A: Editor-First Platform** — this milestone.
- **Phase 6B: Import Pipelines** — SysML v2 and ReqIF reverse codegen.
- **Phase 6C: Document Generation** — `deal build --target docs`.
- **Phase 6D: Stdlib Expansion** — protocols, standards, patterns.
- **Phase 6 Gate** — integrated end-to-end demonstration once 6A-6D are complete.

`REQ-phase-6-1-desktop-editor` should be amended from "Tauri + Next.js" to "Tauri + React + Vite" and from read-only diagrams to editable, viewpoint-constrained canvas projections.

`ADR-graphical-view-projection-and-sync.md` should be amended for the renderer implementation language: Rust-to-WASM for the editor milestone, while preserving the ADR's WebGL2 floor, WebGPU promotion gates, hold-last-good sync contract, `.dealview` model, and edit-through-view rules.

## Open Decisions Closed By This Spec

- Desktop editor is its own milestone.
- React + Vite replaces Next.js for the editor frontend.
- WebGL/WASM renderer is MVP scope.
- Renderer kernel is Rust-to-WASM.
- Semantic diagram editing is MVP scope.
- Canvas handles are the primary semantic editing pattern.
- Both structure and traceability semantic edits are MVP scope.
- `deal dev` is a named local live model daemon.
- `deal dev` includes a lightweight browser diagnostics UI.
- Git-aware safeguards are MVP scope.
- Simulation/check behavior remains DEAL CLI-owned.
- Classification support is MVP scope.
- `.dealview` authoring and validation are MVP scope.
- Three-column workbench is the MVP default layout, with modular internals.
- UI chrome state is user-local; diagram layout is versioned `.dealview`.
- Signed/notarized manual binaries are MVP scope.
- Auto-update is out of MVP scope.
