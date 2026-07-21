//! Dashboard CSS (composed).
pub use super::styles_base::CSS_BASE;
pub use super::styles_panels::CSS_PANELS;

/// Full stylesheet embedded in the dashboard.
pub fn css_full() -> String {
    format!("{}{}", CSS_BASE, CSS_PANELS)
}

/// Back-compat constant — prefer css_full() for new code.
pub const CSS: &str = ""; // filled at render time via css_full in mod
