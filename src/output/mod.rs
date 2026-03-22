pub mod daily;
pub mod explain;
pub mod json;
pub mod session;
pub mod style;
pub mod table;

/// Round to 2 decimal places for JSON cost output.
pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
