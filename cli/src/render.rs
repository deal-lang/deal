//! Diagnostic rendering for human-mode output.
//!
//! Uses `owo-colors` for color styling and `anstream` for TTY-aware
//! ANSI passthrough/stripping based on `--color={auto,always,never}`.
//!
//! Security: diagnostic messages are raw user-controlled strings — they are
//! rendered via `write!` with `{}` format (NOT format-string args from user
//! input). This matches T-02-10 from the plan threat model.
//!
//! Per RESEARCH §Anti-Patterns: NEVER use `println!` — always write through
//! the `anstream` writer so --color controls work correctly.

use owo_colors::OwoColorize;
use std::io::Write;

/// Render a single diagnostic to stderr.
///
/// `source` is the original source text for the file being diagnosed
/// (used to extract source snippets). May be empty if source is unavailable.
///
/// `diagnostic` is a single diagnostic in the Phase 1 JSON shape:
/// `{code, fix_it?, message, notes, secondary_spans, severity, span}`
/// (alphabetical key order per D-18).
///
/// Per RESEARCH §Anti-Patterns: always write through `out`; never `println!`.
pub fn render_diagnostic(
    out: &mut anstream::AutoStream<std::io::Stderr>,
    source: &str,
    diagnostic: &serde_json::Value,
) -> std::io::Result<()> {
    let code = diagnostic["code"].as_str().unwrap_or("E????");
    let severity = diagnostic["severity"].as_str().unwrap_or("err");
    let message = diagnostic["message"].as_str().unwrap_or("unknown error");
    let span_start = diagnostic["span"]["start"].as_u64().unwrap_or(0) as usize;
    let span_end = diagnostic["span"]["end"].as_u64().unwrap_or(span_start as u64) as usize;
    let notes = diagnostic["notes"].as_str().unwrap_or("");

    // Severity label with color.
    match severity {
        "err" => write!(out, "{}", "error".red().bold())?,
        "warn" => write!(out, "{}", "warning".yellow().bold())?,
        "info" => write!(out, "{}", "info".blue().bold())?,
        "hint" => write!(out, "{}", "hint".green().bold())?,
        other => write!(out, "{}", other.red().bold())?,
    }

    // Message and code.
    writeln!(out, ": {} [{}]", message, code.bold())?;

    // Source snippet with caret, if source is available and span is valid.
    if !source.is_empty() && span_start < source.len() {
        let span_end_clamped = span_end.min(source.len());
        render_snippet(out, source, span_start, span_end_clamped)?;
    }

    // Secondary spans.
    if let Some(secondary) = diagnostic["secondary_spans"].as_array() {
        for sec in secondary {
            let sec_label = sec["label"].as_str().unwrap_or("");
            let sec_start = sec["span"]["start"].as_u64().unwrap_or(0) as usize;
            let sec_end = sec["span"]["end"].as_u64().unwrap_or(sec_start as u64) as usize;
            if !source.is_empty() && sec_start < source.len() {
                let sec_end_clamped = sec_end.min(source.len());
                write!(out, "  {} ", "-->".blue().bold())?;
                writeln!(out, "{}", sec_label)?;
                render_snippet(out, source, sec_start, sec_end_clamped)?;
            } else if !sec_label.is_empty() {
                writeln!(out, "  = note: {}", sec_label)?;
            }
        }
    }

    // Notes.
    if !notes.is_empty() {
        writeln!(out, "  = {}: {}", "note".bold(), notes)?;
    }

    // Fix-it suggestion.
    let fix_it = &diagnostic["fix_it"];
    if !fix_it.is_null() {
        let replacement = fix_it["replacement"].as_str().unwrap_or("");
        if !replacement.is_empty() {
            writeln!(out, "  = {}: replace with `{}`", "help".bold(), replacement)?;
        }
    }

    // Trailing blank line to separate diagnostics.
    writeln!(out)?;

    Ok(())
}

/// Render a source snippet with a caret pointing at the span.
///
/// Finds the line containing the span and underlines the relevant range.
fn render_snippet(
    out: &mut anstream::AutoStream<std::io::Stderr>,
    source: &str,
    span_start: usize,
    span_end: usize,
) -> std::io::Result<()> {
    // Find line number and column for span_start.
    let (line_num, col_start) = byte_offset_to_line_col(source, span_start);
    let col_end = if span_end > span_start {
        col_start + (span_end - span_start)
    } else {
        col_start + 1
    };

    // Extract the source line.
    let line_text = source
        .lines()
        .nth(line_num.saturating_sub(1))
        .unwrap_or("");

    // Line number gutter.
    let gutter = format!("{}", line_num);
    let gutter_width = gutter.len();

    // File pointer line.
    writeln!(out, "  {} ---> :{line_num}:{col_start}", " ".repeat(gutter_width).blue().bold())?;

    // Blank gutter line.
    writeln!(out, "  {} {} ", " ".repeat(gutter_width), "|".blue().bold())?;

    // Source line.
    writeln!(out, "  {} {} {}", gutter.blue().bold(), "|".blue().bold(), line_text)?;

    // Caret line.
    let indent = col_start.saturating_sub(1);
    let caret_len = col_end
        .saturating_sub(col_start)
        .max(1)
        .min(line_text.len().saturating_sub(indent) + 1);
    let carets = "^".repeat(caret_len);
    writeln!(
        out,
        "  {} {} {}{}",
        " ".repeat(gutter_width),
        "|".blue().bold(),
        " ".repeat(indent),
        carets.red().bold()
    )?;

    Ok(())
}

/// Convert a byte offset into a (1-based line number, 1-based column) pair.
pub fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.chars().filter(|&c| c == '\n').count() + 1;
    let col = match prefix.rfind('\n') {
        Some(pos) => offset - pos,
        None => offset + 1,
    };
    (line, col)
}
