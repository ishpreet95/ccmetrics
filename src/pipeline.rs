use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::output::style;

/// Whether pipeline output should be shown.
///
/// Checks stderr (where pipeline writes), not stdout (where data goes).
/// Suppressed when stderr is not a terminal (e.g., `2>/dev/null`).
/// Additional suppression (--json, --quiet) is handled in main.rs.
pub fn should_show() -> bool {
    io::stderr().is_terminal()
}

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A pipeline step that shows progress with an animated spinner.
pub struct PipelineStep {
    start: Instant,
    use_color: bool,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl PipelineStep {
    /// Start a new pipeline step with an animated spinner on a background thread.
    pub fn start(label: &str) -> Self {
        let color = style::stderr_color();
        let stop = Arc::new(AtomicBool::new(false));

        let handle = if color {
            let stop_clone = Arc::clone(&stop);
            let label_owned = label.to_string();
            Some(thread::spawn(move || {
                let mut frame = 0usize;
                while !stop_clone.load(Ordering::Relaxed) {
                    let ch = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
                    eprint!(
                        "\r\x1b[2K  {}{}{} {} ...",
                        style::ACCENT,
                        ch,
                        style::RESET,
                        label_owned,
                    );
                    let _ = io::stderr().flush();
                    frame += 1;
                    thread::sleep(Duration::from_millis(80));
                }
            }))
        } else {
            eprint!("  ... {} ...", label);
            let _ = io::stderr().flush();
            None
        };

        PipelineStep {
            start: Instant::now(),
            use_color: color,
            stop,
            handle,
        }
    }

    /// Stop the spinner thread and wait for it to finish.
    fn stop_spinner(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Complete the step with a result summary.
    pub fn done(mut self, result: &str) {
        self.stop_spinner();
        let ms = self.start.elapsed().as_millis();

        if self.use_color {
            eprint!("\r\x1b[2K");
            eprintln!(
                "  {}✓{} {} {}({}ms){}",
                style::GREEN,
                style::RESET,
                result,
                style::DIM,
                ms,
                style::RESET,
            );
        } else {
            eprintln!();
            eprintln!("  [ok] {} ({}ms)", result, ms);
        }
    }

    /// Complete the step with a warning.
    #[allow(dead_code)]
    pub fn warn(mut self, result: &str) {
        self.stop_spinner();
        let ms = self.start.elapsed().as_millis();

        if self.use_color {
            eprint!("\r\x1b[2K");
            eprintln!(
                "  {}!{} {} {}({}ms){}",
                style::YELLOW,
                style::RESET,
                result,
                style::DIM,
                ms,
                style::RESET,
            );
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
    if style::stderr_color() {
        eprintln!("  {}{}{}", style::DIM, "─".repeat(60), style::RESET);
    } else {
        eprintln!("  {}", "─".repeat(60));
    }
    eprintln!();
}
