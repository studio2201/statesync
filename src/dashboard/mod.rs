//! Dashboard UI rendering module.

use maud::{DOCTYPE, Markup, html};

pub mod styles;
pub mod scripts;
pub mod scripts_actions;
pub mod scripts_modals;

/// Concatenates the embedded Rust JavaScript string slices into a single string for HTML insertion.
pub fn render_full_js() -> String {
    format!(
        "{}{}{}",
        scripts::JS_CORE,
        scripts_actions::JS_ACTIONS,
        scripts_modals::JS_MODALS
    )
}

/// Renders the complete HTML dashboard markup using Maud templates.
pub fn render_dashboard() -> Markup {
    let full_js = render_full_js();
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                meta name="theme-color" content="#0b0f14";
                title { "StateSync" }
                link rel="manifest" href="/manifest.json";
                link rel="apple-touch-icon" href="/icon.svg";
                link rel="shortcut icon" href="/favicon.jpg" type="image/jpeg";
                style { (maud::PreEscaped(styles::CSS)) }
            }
            body {
                div class="container" {
                    div class="header" {
                        div class="brand" {
                            img src="/favicon.jpg" alt="";
                            span { "StateSync" }
                        }
                        div class="actions" {
                            button class="btn" id="refreshUsersBtn" onclick="refreshUsers()" { "Refresh users" }
                            button class="btn btn-primary" id="forceSyncBtn" onclick="forceSync()" { "Force sync" }
                            button class="btn" onclick="openSettingsModal()" { "Settings" }
                            button class="btn btn-primary" onclick="openServerModal(-1)" { "Add server" }
                        }
                    }

                    div id="lastFullSyncBanner" class="banner" {}
                    div id="forceSyncLive" class="banner" style="display:none;border-color:var(--accent)" {
                        div style="flex:1" {
                            div style="display:flex;justify-content:space-between;gap:10px;margin-bottom:6px" {
                                strong style="color:var(--bright)" { "Full sync in progress" }
                                span id="fsProgressText" style="color:var(--accent)" {}
                            }
                            progress id="fsProgressBar" value="0" max="100" style="width:100%;height:8px" {}
                            div id="fsCurrentUser" class="form-hint" {}
                        }
                        button class="btn btn-danger" id="fsCancelBtn" onclick="cancelForceSync()" { "Cancel" }
                    }

                    div class="row-grid" {
                        div class="card" {
                            h2 { "Mapped users" }
                            div id="syncedUsers" {}
                            div id="forceSyncStatus" class="form-hint" style="margin-top:10px" {}
                        }
                        div class="stack" {
                            div class="card" {
                                h2 { "Active streams" }
                                div id="activeSessions" {
                                    div class="empty" { "No one is playing anything right now." }
                                }
                            }
                            div class="card" {
                                h2 { "Media servers" }
                                div id="serverList" {}
                            }
                        }
                    }

                    div class="card" {
                        div style="display:flex;justify-content:space-between;align-items:center;gap:10px;margin-bottom:12px" {
                            h2 style="margin:0" { "Activity log" }
                            button class="btn" id="toggleLogsBtn" onclick="toggleLogs()" { "Collapse" }
                        }
                        div class="log-feed" id="syncLogs" {}
                    }

                    div class="footer" {
                        div id="versionFooter" {}
                        div style="display:flex;gap:8px;align-items:center" {
                            label for="themeSelector" { "Theme" }
                            select id="themeSelector" onchange="setTheme(this.value)" {
                                option value="cyberpunk" { "Default" }
                                option value="matrix" { "Green" }
                                option value="amber" { "Amber" }
                                option value="dracula" { "Purple" }
                            }
                        }
                    }
                }

                div class="modal" id="serverModal" style="display:none" {
                    div class="modal-content" {
                        h2 id="modalTitle" { "Add server" }
                        form id="serverForm" {
                            input type="hidden" id="serverType" value="jellyfin";
                            input type="hidden" id="serverDirection" value="both";
                            input type="hidden" id="serverName" value="";

                            div class="form-group" {
                                label { "Type" }
                                div class="btn-group" {
                                    button type="button" class="btn-radio active" id="btnJellyfin" onclick="pickType('jellyfin')" { "Jellyfin" }
                                    button type="button" class="btn-radio" id="btnEmby" onclick="pickType('emby')" { "Emby" }
                                }
                            }
                            div class="form-group" {
                                label { "Server address" }
                                input type="text" id="serverUrl" placeholder="http://192.168.1.50:8096 or 192.168.1.50:8096" required {};
                                p class="form-hint" { "Any reachable IP or hostname. Scheme is optional (defaults to http)." }
                            }
                            div class="form-group" {
                                label { "API key" }
                                input type="password" id="serverKey" required {};
                            }
                            div class="form-group" {
                                label { "Sync direction" }
                                div class="btn-group" {
                                    button type="button" class="btn-radio active" data-dir="both" onclick="pickDirection('both')" { "Both ways" }
                                    button type="button" class="btn-radio" data-dir="send" onclick="pickDirection('send')" { "Send only" }
                                    button type="button" class="btn-radio" data-dir="receive" onclick="pickDirection('receive')" { "Receive only" }
                                }
                            }
                            div class="modal-actions" {
                                button type="button" class="btn" onclick="testConnection()" { "Test connection" }
                                div class="right" {
                                    button type="button" class="btn" onclick="closeModal('serverModal')" { "Cancel" }
                                    button type="submit" class="btn btn-primary" { "Save" }
                                }
                            }
                        }
                    }
                }

                div class="modal" id="settingsModal" style="display:none" {
                    div class="modal-content" style="max-width:520px" {
                        h2 { "Settings" }
                        div class="form-group" {
                            label { "Sync threshold (seconds)" }
                            input type="number" id="syncThreshold" min="1" max="60" value="5" {};
                            p class="form-hint" { "Ignore near-duplicate progress updates within this window." }
                        }
                        div class="form-group" {
                            label { "Username mappings" }
                            textarea id="cfgUserMappings" rows="5" placeholder="alice, Alice, alice_jf&#10;bob, Robert" {};
                            p class="form-hint" { "One group per line, comma-separated names that should match across servers." }
                        }
                        div class="modal-actions" {
                            div {}
                            div class="right" {
                                button type="button" class="btn" onclick="closeModal('settingsModal')" { "Cancel" }
                                button type="button" class="btn btn-primary" onclick="saveSettings()" { "Save settings" }
                            }
                        }
                    }
                }

                div class="toast" id="toast" {}

                script { (maud::PreEscaped(full_js)) }
            }
        }
    }
}

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
        assert!(html_str.contains("Active streams"));
        assert!(html_str.contains("Media servers"));
        assert!(html_str.contains("Activity log"));
        // No decorative bracket chrome
        assert!(!html_str.contains("[ MAPPED USERS ]"));
    }
}
