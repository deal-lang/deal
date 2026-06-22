//! ADR-0004 P5 WS-A: the import-closure walker moved to the shared
//! `deal-closure` crate so the LSP reuses the identical reachability logic.
//! This module re-exports it so existing `crate::closure::*` call-sites
//! (main.rs `plan_load_from_paths`, `closure_files`, `LoadPlan`, …) keep
//! compiling unchanged. The walker's unit tests live in `deal-closure`.

pub use deal_closure::*;
