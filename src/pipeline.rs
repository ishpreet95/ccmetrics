use std::io::{self, IsTerminal, Write};
use std::time::Instant;

/// Whether pipeline output should be shown.
///
/// Checks stderr (where pipeline writes), not stdout (where data goes).
/// Suppressed when stderr is not a terminal (e.g., `2>/dev/null`).
/// Additional suppression (--json, --quiet) is handled in main.rs.
pub fn should_show() -> bool {
    io::stderr().is_terminal()
}

/// Whether to use ANSI color/escape codes.
fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err()
}

const SPINNER_CHAR: char = '⠋';

/// A pipeline step that shows progress with a spinner.
pub struct PipelineStep {
    #[allow(dead_code)]
    label: String,
    start: Instant,
    use_color: bool,
}

impl PipelineStep {
    /// Start a new pipeline step with a spinner.
    pub fn start(label: &str) -> Self {
        let color = use_color();
        let step = PipelineStep {
            label: label.to_string(),
            start: Instant::now(),
            use_color: color,
        };

        if color {
            eprint!("  \x1b[36m{}\x1b[0m {} ...", SPINNER_CHAR, label);
        } else {
            eprint!("  ... {} ...", label);
        }
        let _ = io::stderr().flush();

        step
    }

    /// Complete the step with a result summary.
    pub fn done(self, result: &str) {
        let elapsed = self.start.elapsed();
        let ms = elapsed.as_millis();

        if self.use_color {
            eprint!("\r\x1b[2K");
            eprintln!("  \x1b[32m✓\x1b[0m {} \x1b[2m({}ms)\x1b[0m", result, ms);
        } else {
            eprintln!();
            eprintln!("  [ok] {} ({}ms)", result, ms);
        }
    }

    /// Complete the step with a warning.
    #[allow(dead_code)]
    pub fn warn(self, result: &str) {
        let elapsed = self.start.elapsed();
        let ms = elapsed.as_millis();

        if self.use_color {
            eprint!("\r\x1b[2K");
            eprintln!("  \x1b[33m!\x1b[0m {} \x1b[2m({}ms)\x1b[0m", result, ms);
        } else {
            eprintln!();
            eprintln!("  [warn] {} ({}ms)", result, ms);
        }
    }
}

/// Print the pipeline header.
pub fn header() {
    eprintln!();
}

/// Print the pipeline separator before dashboard output.
pub fn separator() {
    if use_color() {
        eprintln!("  \x1b[2m{}\x1b[0m", "─".repeat(60));
    } else {
        eprintln!("  {}", "─".repeat(60));
    }
    eprintln!();
}
