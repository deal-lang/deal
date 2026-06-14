//! Centralized human-mode CLI presentation layer (Phase 6 CLI richness).
//!
//! Every subcommand routes its rich, human-facing output (the staged phase
//! header, simulation spinners + timings, and the `--verify` results table)
//! through a single `Reporter` so the look stays consistent — and, far more
//! importantly, so decoration can **never** leak into the machine contract.
//!
//! ## Invariants this layer protects
//!
//! * `--json` (D-32 envelope) and raw payloads on stdout are assembled by the
//!   existing emit paths, NOT through `Reporter` — the reporter only ever
//!   writes human chrome (to stderr, or to the verify report stream).
//! * `--color=never` and non-TTY output carry zero ANSI: `paint` is a no-op
//!   when `color` is false, and callers still pass through the `anstream`
//!   writer which strips as a second gate.
//! * Live spinners (`indicatif`) are constructed ONLY when `animate` is true —
//!   an interactive TTY, color enabled, and `CI` unset. In pipelines / CI the
//!   same information degrades to a single static line per simulation, so logs
//!   stay greppable and deterministic.
//!
//! Per the Phase 2 anti-pattern (`render.rs`): never `println!`; always write
//! through the caller-supplied writer.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use unicode_width::UnicodeWidthStr;

/// Color preference mirrored from the CLI `--color` flag.
///
/// Kept local to the `deal` library crate so `verify.rs` / `simulate.rs` can
/// build a `Reporter` without importing the binary's `ColorMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorPref {
    Auto,
    Always,
    Never,
}

/// Semantic color roles. Maps to the CLI Color Contract (04-UI-SPEC):
/// red error, yellow warning, blue note, green success, cyan emphasis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Plain,
    Green,
    Red,
    Yellow,
    Blue,
    Cyan,
    Dim,
    Bold,
}

/// A single table cell: its plain text plus the ink to paint it with.
///
/// Width is always measured on the *plain* text (ANSI codes have zero display
/// width but non-zero byte length), so padding stays correct after coloring.
pub struct Cell {
    pub text: String,
    pub ink: Ink,
}

impl Cell {
    pub fn new(text: impl Into<String>, ink: Ink) -> Self {
        Cell { text: text.into(), ink }
    }
    /// Convenience for an uncolored cell.
    pub fn plain(text: impl Into<String>) -> Self {
        Cell::new(text, Ink::Plain)
    }
}

/// Human-mode presentation surface.
pub struct Reporter {
    /// Apply ANSI styling.
    color: bool,
    /// Draw live, repainting spinners. Implies an interactive TTY.
    animate: bool,
}

impl Reporter {
    /// Build a reporter for human (non-JSON) output.
    ///
    /// `color` resolves `--color`: `Always` => on, `Never` => off,
    /// `Auto` => on iff stderr is a TTY. Animation additionally requires a TTY
    /// and the absence of a `CI` env var, so spinners never fire in pipelines.
    pub fn new(pref: ColorPref) -> Self {
        let is_tty = std::io::stderr().is_terminal();
        let color = match pref {
            ColorPref::Always => true,
            ColorPref::Never => false,
            ColorPref::Auto => is_tty,
        };
        let in_ci = std::env::var_os("CI").is_some();
        let animate = color && is_tty && !in_ci;
        Reporter { color, animate }
    }

    /// True when live spinners / progress bars are safe to draw.
    pub fn animated(&self) -> bool {
        self.animate
    }

    /// True when ANSI styling is enabled.
    pub fn color_enabled(&self) -> bool {
        self.color
    }

    /// Paint `text` with `ink`, or return it unchanged when color is disabled.
    pub fn paint(&self, text: &str, ink: Ink) -> String {
        if !self.color || ink == Ink::Plain {
            return text.to_string();
        }
        match ink {
            Ink::Plain => text.to_string(),
            Ink::Green => text.green().to_string(),
            Ink::Red => text.red().bold().to_string(),
            Ink::Yellow => text.yellow().bold().to_string(),
            Ink::Blue => text.blue().bold().to_string(),
            Ink::Cyan => text.cyan().to_string(),
            Ink::Dim => text.dimmed().to_string(),
            Ink::Bold => text.bold().to_string(),
        }
    }

    /// Emit the project banner, e.g. `deal 0.1 · checking 37 files`.
    pub fn banner(&self, w: &mut impl Write, summary: &str) -> std::io::Result<()> {
        let version = env!("CARGO_PKG_VERSION");
        let line = format!("deal {version} · {summary}");
        writeln!(w, "{}", self.paint(&line, Ink::Dim))
    }

    /// Render a block of phase rows with aligned columns:
    ///
    /// ```text
    /// ▸ parse     37 files            ok
    /// ▸ resolve   218 symbols         ok
    /// ▸ units     dimensional algebra ok
    /// ```
    ///
    /// Each row is `(label, detail, status)`. `status` is painted green when
    /// `Some`, and omitted when `None` (e.g. an in-progress phase).
    pub fn phases(
        &self,
        w: &mut impl Write,
        rows: &[(&str, String, Option<&str>)],
    ) -> std::io::Result<()> {
        let label_w = rows.iter().map(|(l, ..)| l.width()).max().unwrap_or(0);
        let detail_w = rows.iter().map(|(_, d, _)| d.width()).max().unwrap_or(0);
        for (label, detail, status) in rows {
            let marker = self.paint("▸", Ink::Cyan);
            let label_p = pad(label, label_w);
            match status {
                Some(s) => {
                    let detail_p = pad(detail, detail_w);
                    writeln!(
                        w,
                        "{marker} {} {} {}",
                        self.paint(&label_p, Ink::Cyan),
                        detail_p,
                        self.paint(s, Ink::Green),
                    )?;
                }
                None => {
                    writeln!(w, "{marker} {} {}", self.paint(&label_p, Ink::Cyan), detail)?;
                }
            }
        }
        Ok(())
    }

    /// Compute the per-column display widths needed to align `rows`.
    pub fn col_widths(rows: &[Vec<Cell>]) -> Vec<usize> {
        let mut widths: Vec<usize> = Vec::new();
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                let w = cell.text.width();
                if i >= widths.len() {
                    widths.push(w);
                } else if w > widths[i] {
                    widths[i] = w;
                }
            }
        }
        widths
    }

    /// Render one table row, left-aligning each cell to `widths` and separating
    /// columns by two spaces. The final cell is not padded.
    pub fn row(
        &self,
        w: &mut impl Write,
        widths: &[usize],
        indent: usize,
        cells: &[Cell],
    ) -> std::io::Result<()> {
        let mut line = " ".repeat(indent);
        for (i, cell) in cells.iter().enumerate() {
            let painted = self.paint(&cell.text, cell.ink);
            line.push_str(&painted);
            if i + 1 < cells.len() {
                let col_w = widths.get(i).copied().unwrap_or(0);
                let pad_n = col_w.saturating_sub(cell.text.width()) + 2;
                line.push_str(&" ".repeat(pad_n));
            }
        }
        writeln!(w, "{}", line)
    }

    /// Create a steady-tick spinner with `msg`, or `None` when not animating.
    ///
    /// When `None`, callers should just print a static result line on finish.
    pub fn spinner(&self, msg: &str) -> Option<ProgressBar> {
        if !self.animate {
            return None;
        }
        let pb = ProgressBar::new_spinner();
        // Unwrap is safe: the template is a compile-time constant literal.
        let style = ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner());
        pb.set_style(style.tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "));
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(Duration::from_millis(80));
        Some(pb)
    }
}

/// Pad `s` on the right to `width` display columns (no-op if already wider).
fn pad(s: &str, width: usize) -> String {
    let w = s.width();
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_pref_disables_color() {
        let r = Reporter::new(ColorPref::Never);
        assert!(!r.color_enabled());
        assert!(!r.animated());
        // paint must be a pure passthrough — no ANSI bytes.
        assert_eq!(r.paint("error", Ink::Red), "error");
    }

    #[test]
    fn always_pref_paints_but_only_animates_on_tty() {
        let r = Reporter::new(ColorPref::Always);
        assert!(r.color_enabled());
        assert!(r.paint("ok", Ink::Green).contains("ok"));
        // Tests do not run under a TTY, so animation stays off regardless.
        assert!(!r.animated());
    }

    #[test]
    fn col_widths_track_max_per_column() {
        let rows = vec![
            vec![Cell::plain("PASS"), Cell::plain("REQ_MOT_001")],
            vec![Cell::plain("FAIL"), Cell::plain("REQ_BAT_004_LONG")],
        ];
        let w = Reporter::col_widths(&rows);
        assert_eq!(w[0], 4);
        assert_eq!(w[1], "REQ_BAT_004_LONG".len());
    }

    #[test]
    fn pad_handles_wide_chars() {
        // A CJK char counts as two display columns.
        assert_eq!(pad("中", 4), "中  ");
        assert_eq!(pad("ab", 2), "ab");
    }
}
