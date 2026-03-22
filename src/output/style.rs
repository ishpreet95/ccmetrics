use std::io::IsTerminal;

// Boreal Command palette — ANSI 256 codes
pub const ACCENT: &str = "\x1b[38;5;151m"; // #afd7af — sage-mint (heroes, bars)
pub const FROST: &str = "\x1b[38;5;254m"; // #e4e4e4 — near-white (data values)
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const ITALIC: &str = "\x1b[3m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RED: &str = "\x1b[31m";
pub const RESET: &str = "\x1b[0m";

/// Whether stdout supports color (for dashboard output).
pub fn stdout_color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

/// Whether stderr supports color (for pipeline output).
pub fn stderr_color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()
}

/// Wrap text in accent color if color is enabled.
#[allow(dead_code)]
pub fn accent(text: &str, color: bool) -> String {
    if color {
        format!("{ACCENT}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Wrap text in bold if color is enabled.
#[allow(dead_code)]
pub fn bold(text: &str, color: bool) -> String {
    if color {
        format!("{BOLD}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Wrap text in dim if color is enabled.
pub fn dim(text: &str, color: bool) -> String {
    if color {
        format!("{DIM}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Bold + accent — for hero numbers (the single most important value).
pub fn hero(text: &str, color: bool) -> String {
    if color {
        format!("{BOLD}{ACCENT}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Chip style — accent background with dark foreground for unmissable hero numbers.
/// Uses ANSI: accent background (48;5;151) + black text (30m) + bold.
pub fn chip(text: &str, color: bool) -> String {
    if color {
        format!("{BOLD}\x1b[48;5;151m\x1b[38;5;16m {text} {RESET}")
    } else {
        format!("[ {text} ]")
    }
}

/// Accent + bold — for section sub-headers (By thread, By model).
pub fn subheader(text: &str, color: bool) -> String {
    if color {
        format!("{BOLD}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Dim text for secondary values (percentages, descriptions in rows).
pub fn secondary(text: &str, color: bool) -> String {
    if color {
        format!("{DIM}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Frost-tinted data values (dollar amounts, token counts in rows).
pub fn value(text: &str, color: bool) -> String {
    if color {
        format!("{FROST}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Italic for inline descriptions.
pub fn description(text: &str, color: bool) -> String {
    if color {
        format!("{DIM}{ITALIC}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Render a proportional bar: ██████░░░░░░░░░░
/// Returns empty string if width < 1.
/// Guarantees at least 1 filled char when fraction > 0.
pub fn render_bar(fraction: f64, width: usize, color: bool) -> String {
    if width == 0 {
        return String::new();
    }
    let clamped = fraction.clamp(0.0, 1.0);
    let mut filled = (clamped * width as f64).round() as usize;
    if clamped > 0.0 && filled == 0 {
        filled = 1; // tiny values don't vanish
    }
    let empty = width.saturating_sub(filled);
    if color {
        format!(
            "{ACCENT}{}{RESET}{DIM}{}{RESET}",
            "█".repeat(filled),
            "░".repeat(empty),
        )
    } else {
        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }
}

/// Insight severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsightLevel {
    Good,
    Note,
    Warn,
}

/// Render a contextual insight line: ● message
pub fn render_insight(level: InsightLevel, msg: &str, color: bool) -> String {
    if color {
        let color_code = match level {
            InsightLevel::Good => GREEN,
            InsightLevel::Note => YELLOW,
            InsightLevel::Warn => RED,
        };
        format!("  {color_code}●{RESET} {DIM}{msg}{RESET}")
    } else {
        let label = match level {
            InsightLevel::Good => "[good]",
            InsightLevel::Note => "[note]",
            InsightLevel::Warn => "[warn]",
        };
        format!("  {label} {msg}")
    }
}

/// Get terminal width from COLUMNS env var, default 80.
pub fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(80)
}

/// Whether to show bars (terminal wide enough).
pub fn show_bars(width: usize) -> bool {
    width >= 70
}

/// Bar width, clamped between 10 and 30.
pub fn bar_width(terminal_width: usize) -> usize {
    // Bars occupy roughly 40% of the remaining space after labels
    let available = terminal_width.saturating_sub(40);
    available.clamp(10, 30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_bar_full() {
        let bar = render_bar(1.0, 10, false);
        assert_eq!(bar, "██████████");
    }

    #[test]
    fn test_render_bar_empty() {
        let bar = render_bar(0.0, 10, false);
        assert_eq!(bar, "░░░░░░░░░░");
    }

    #[test]
    fn test_render_bar_half() {
        let bar = render_bar(0.5, 10, false);
        assert_eq!(bar, "█████░░░░░");
    }

    #[test]
    fn test_render_bar_tiny_nonzero() {
        // Tiny fraction should still show 1 filled char
        let bar = render_bar(0.001, 10, false);
        assert_eq!(bar, "█░░░░░░░░░");
    }

    #[test]
    fn test_render_bar_zero_width() {
        let bar = render_bar(0.5, 0, false);
        assert_eq!(bar, "");
    }

    #[test]
    fn test_render_bar_with_color() {
        let bar = render_bar(1.0, 3, true);
        assert!(bar.contains(ACCENT));
        assert!(bar.contains(RESET));
        assert!(bar.contains("███"));
    }

    #[test]
    fn test_render_insight_no_color() {
        let s = render_insight(InsightLevel::Good, "test", false);
        assert_eq!(s, "  [good] test");
        let s = render_insight(InsightLevel::Note, "test", false);
        assert_eq!(s, "  [note] test");
        let s = render_insight(InsightLevel::Warn, "test", false);
        assert_eq!(s, "  [warn] test");
    }

    #[test]
    fn test_render_insight_with_color() {
        let s = render_insight(InsightLevel::Good, "cache hit", true);
        assert!(s.contains(GREEN));
        assert!(s.contains("●"));
        assert!(s.contains("cache hit"));
    }

    #[test]
    fn test_bar_width_clamping() {
        assert_eq!(bar_width(50), 10); // min
        assert_eq!(bar_width(200), 30); // max
    }

    #[test]
    fn test_show_bars_threshold() {
        assert!(!show_bars(69));
        assert!(show_bars(70));
        assert!(show_bars(120));
    }

    #[test]
    fn test_accent_no_color() {
        assert_eq!(accent("test", false), "test");
    }

    #[test]
    fn test_bold_with_color() {
        let s = bold("hero", true);
        assert!(s.contains(BOLD));
        assert!(s.contains("hero"));
    }

    #[test]
    fn test_dim_with_color() {
        let s = dim("subtle", true);
        assert!(s.contains(DIM));
        assert!(s.contains("subtle"));
    }
}
