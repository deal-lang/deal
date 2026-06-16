//! `deal-lsp-gen-golden` — committed cargo bin that regenerates the
//! semantic-tokens golden file. Plan 03-04 Task 3 / Issue 6 resolution:
//! the golden file is committed to git and the regression test reads it
//! via `include_str!`; this binary is the ONLY path that ever writes
//! the golden. The test does NOT have an `UPDATE_GOLDEN` env var.
//!
//! Invocation:
//!
//!   cargo run --release -p deal-lsp --bin deal-lsp-gen-golden -- \
//!       <input.deal> <output.expected.json>
//!
//! Defaults (no argv):
//!   input  = tests/showcase/packages/vehicle/battery.deal
//!   output = lsp/tests/golden/semantic_tokens/BatteryPack.deal.expected.json
//!
//! Sets `DEAL_LSP_DETERMINISTIC_RESULT_ID=1` so the encoder emits
//! `result_id = "golden"` — the committed golden is byte-stable.

use deal_lsp::semantic_tokens;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tests/showcase/packages/vehicle/battery.deal"));
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("lsp/tests/golden/semantic_tokens/BatteryPack.deal.expected.json")
    });

    let source = std::fs::read(&input)?;
    let filename = input.to_string_lossy().into_owned();

    // SAFETY: single-threaded process, set BEFORE the encoder runs.
    // The env var only changes result_id; no other state.
    std::env::set_var("DEAL_LSP_DETERMINISTIC_RESULT_ID", "1");

    let tokens = semantic_tokens::encode_for_source(&source, &filename);

    // serde_json::to_string_pretty emits alphabetically-ordered keys by
    // default for derive(Serialize) structs that use #[serde(derive)]
    // ordering — SemanticTokens emits `data` then `result_id` as in
    // the LSP type definition. Add a trailing newline so the file is
    // POSIX-clean.
    let mut json = serde_json::to_string_pretty(&tokens).map_err(std::io::Error::other)?;
    json.push('\n');

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, json.as_bytes())?;
    eprintln!(
        "gen-golden: wrote {} ({} tokens, {} bytes)",
        output.display(),
        tokens.data.len() / 5,
        json.len()
    );
    Ok(())
}
