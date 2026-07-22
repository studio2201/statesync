//! Dashboard UI rendering module.

pub mod scripts;
pub mod scripts_actions;
pub mod scripts_config_save;
pub mod scripts_force_ui;
pub mod scripts_map_settings;
pub mod scripts_server_form;
pub mod scripts_sessions_users;
pub mod scripts_user_actions;
pub mod styles;
pub mod styles_base;
pub mod styles_panels;

/// Concatenates the embedded Rust JavaScript string slices into a single string for HTML insertion.
pub fn render_full_js() -> String {
    format!(
        "{}{}{}{}{}{}{}{}",
        scripts::JS_CORE,
        scripts_sessions_users::JS_SESSIONS_USERS,
        scripts_actions::JS_ACTIONS,
        scripts_server_form::JS_SERVER_FORM,
        scripts_map_settings::JS_MAP_SETTINGS,
        scripts_config_save::JS_CONFIG_SAVE,
        scripts_force_ui::JS_FORCE_UI,
        scripts_user_actions::JS_USER_ACTIONS
    )
}

pub mod dashboard_how;
pub mod dashboard_page;

pub use dashboard_page::render_dashboard;

#[cfg(test)]
mod tests {
    use super::render_dashboard;

    #[test]
    fn test_render_dashboard_contains_title() {
        let html_str = render_dashboard().into_string();
        assert!(html_str.contains("<title>StateSync</title>"));
    }

    #[test]
    fn test_render_dashboard_contains_headings() {
        let html_str = render_dashboard().into_string();
        assert!(html_str.contains("Mapped users"));
        assert!(html_str.contains("Now playing"));
        assert!(html_str.contains("Media servers"));
        assert!(html_str.contains("Activity log"));
        assert!(html_str.contains("How sync works"));
        assert!(html_str.contains("/favicon.jpg"));
        assert!(html_str.contains("userActionsModal"));
        assert!(html_str.contains("openUserActionsModal"));
        // No decorative bracket chrome
        assert!(!html_str.contains("[ MAPPED USERS ]"));
    }
}
