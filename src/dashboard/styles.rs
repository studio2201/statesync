//! Dashboard CSS (composed from base + panels).

pub use super::styles_base::CSS_BASE;
pub use super::styles_panels::CSS_PANELS;

/// Full stylesheet embedded in the dashboard HTML.
pub fn css_full() -> String {
    format!("{}{}", CSS_BASE, CSS_PANELS)
}
