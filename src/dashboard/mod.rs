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
                link rel="icon" href="/favicon.jpg" type="image/jpeg";
                link rel="shortcut icon" href="/favicon.jpg" type="image/jpeg";
                link rel="apple-touch-icon" href="/favicon.jpg";
                style { (maud::PreEscaped(styles::CSS)) }
            }
            body {
                div class="container" {
                    div class="header" {
                        div class="brand" {
                            img src="/favicon.jpg" alt="StateSync" width="32" height="32";
                            span { "StateSync" }
                        }
                        div class="actions" {
                            button class="btn" id="refreshUsersBtn" onclick="refreshUsers()" { "Refresh users" }
                            button class="btn" id="previewForceBtn" onclick="forceSync(true)" { "Preview force" }
                            button class="btn btn-primary" id="forceSyncBtn" onclick="forceSync(false)" { "Force sync" }
                            button class="btn" onclick="openSettingsModal()" { "Settings" }
                            button class="btn btn-primary" onclick="openServerModal(-1)" { "Add server" }
                        }
                    }

                    div id="lastFullSyncBanner" class="banner" {}
                    div id="forceSyncLive" class="banner banner-live" style="display:none" {
                        div style="flex:1" {
                            div style="display:flex;justify-content:space-between;gap:10px;margin-bottom:6px;align-items:center" {
                                strong id="fsStoryTitle" style="color:var(--bright)" { "Force sync running" }
                                span id="fsProgressText" style="color:var(--accent)" {}
                            }
                            progress id="fsProgressBar" value="0" max="100" style="width:100%;height:8px" {}
                            div id="fsCurrentUser" class="form-hint" {}
                            div id="fsStoryDetail" class="form-hint" style="margin-top:6px;line-height:1.5" {}
                        }
                        button class="btn btn-danger" id="fsCancelBtn" onclick="cancelForceSync()" { "Cancel" }
                    }

                    div class="card how-sync-card" {
                        div style="display:flex;justify-content:space-between;align-items:center;gap:10px;margin-bottom:10px;flex-wrap:wrap" {
                            h2 style="margin:0" { "How sync works" }
                            button class="btn" id="toggleHowSyncBtn" onclick="toggleHowSync()" { "Collapse" }
                        }
                        div id="howSyncBody" {
                            p class="how-lead" {
                                "StateSync never moves video files. It only copies "
                                strong { "watched, resume point, and favorites" }
                                " between Emby and Jellyfin (and same-type pairs)."
                            }
                            div class="how-grid" {
                                div class="how-step" {
                                    div class="how-num" { "1" }
                                    div class="how-title" { "Connect" }
                                    p { "You add each server’s address and API key. StateSync opens a live event stream to every server that can send updates." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "2" }
                                    div class="how-title" { "Match people" }
                                    p { "Same username on both servers matches automatically. Different names need a manual link (Link users)." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "3" }
                                    div class="how-title" { "Match titles" }
                                    p { "Items are matched by IMDb / TMDb IDs — not by file path or library folder. Same movie, different libraries, still works." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "4" }
                                    div class="how-title" { "Live sync" }
                                    p { "Play, pause, finish, or heart a title → StateSync pushes played, position, and/or favorites (see Settings). Near-duplicates within the threshold are ignored." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "5" }
                                    div class="how-title" { "Force sync" }
                                    p { "Historical backfill (or Preview force with no writes). Skips already matched. Optional user allowlist. Phases + skip reasons in the banner." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "6" }
                                    div class="how-title" { "Clear watched" }
                                    p { "Per-user button on Mapped users: wipes watched flags for that person on every server. Dedicated action — not force sync. Confirm carefully." }
                                }
                            }
                            div class="how-legend" {
                                span { strong { "Live" } " — event stream open; ready for plays" }
                                span { strong { "Checking access" } " — testing API key" }
                                span { strong { "Loading data" } " — fetching users / library index" }
                                span { strong { "Connecting" } " — opening the link" }
                                span { strong { "Reconnecting" } " — link dropped; trying again" }
                                span { strong { "Offline" } " — no link right now" }
                                span { strong { "Failed" } " — last attempt errored" }
                            }
                        }
                    }

                    div class="row-grid" {
                        div class="card" {
                            div style="display:flex;justify-content:space-between;align-items:center;gap:10px;margin-bottom:12px" {
                                h2 style="margin:0" { "Mapped users" }
                                button class="btn" onclick="openMapUsersModal()" { "Link users" }
                            }
                            div id="syncedUsers" {}
                            div id="forceSyncStatus" class="form-hint" style="margin-top:10px" {}
                        }
                        div class="stack" {
                            div class="card" {
                                h2 { "Now playing" }
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
                        div style="display:flex;justify-content:space-between;align-items:center;gap:10px;margin-bottom:12px;flex-wrap:wrap" {
                            h2 style="margin:0" { "Activity log" }
                            div style="display:flex;gap:8px" {
                                button class="btn" id="copyLogsBtn" onclick="copyActivityLog()" { "Copy log" }
                                button class="btn" id="toggleLogsBtn" onclick="toggleLogs()" { "Collapse" }
                            }
                        }
                        div class="log-feed" id="syncLogs" {}
                    }

                    div class="footer" {
                        div id="versionFooter" {}
                    }
                }

                div class="modal" id="serverModal" style="display:none" {
                    div class="modal-content" {
                        h2 id="modalTitle" { "Add server" }
                        form id="serverForm" {
                            input type="hidden" id="serverType" value="";
                            input type="hidden" id="serverDirection" value="both";
                            input type="hidden" id="serverName" value="";

                            div class="form-group" {
                                label { "Server address" }
                                input type="text" id="serverUrl" placeholder="http://emby-or-jellyfin:8096" required {};
                                p class="form-hint" { "Any form works — full browser link, or just host:port. We strip paths automatically." }
                                p class="form-hint" id="serverTypeHint" { "Emby vs Jellyfin is detected automatically when you test or save." }
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
                            label { "Live sync" }
                            p class="form-hint" style="margin-bottom:8px" { "While people watch — what to copy as events happen." }
                            label class="check-row" { input type="checkbox" id="syncLivePlayed" checked; " Played (mark watched)" }
                            label class="check-row" { input type="checkbox" id="syncLivePosition" checked; " Position (resume point)" }
                            label class="check-row" { input type="checkbox" id="syncLiveFavorites" checked; " Favorites (heart)" }
                        }
                        div class="form-group" {
                            label { "Force sync" }
                            p class="form-hint" style="margin-bottom:8px" { "Historical backfill when you press Force sync." }
                            label class="check-row" { input type="checkbox" id="syncForcePlayed" checked; " Played history" }
                            label class="check-row" { input type="checkbox" id="syncForcePosition" checked; " In-progress positions" }
                            label class="check-row" { input type="checkbox" id="syncForceFavorites" checked; " Favorites" }
                            p class="form-hint" { "Force only pushes when the target is missing that state. Use Preview force to count without writing." }
                        }
                        div class="form-group" {
                            label { "User allowlist (optional)" }
                            textarea id="cfgUserAllowlist" rows="3" placeholder="alice&#10;bob" {};
                            p class="form-hint" { "Empty = sync everyone. One name per line (or commas). Linked aliases of allowlisted people are included." }
                        }
                        div class="form-group" {
                            label { "Username mappings (advanced text)" }
                            textarea id="cfgUserMappings" rows="4" placeholder="alice, alice_jf&#10;bob, Robert" {};
                            p class="form-hint" { "Or use Link users for a visual picker. One group per line, comma-separated names." }
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

                div class="modal" id="mapUsersModal" style="display:none" {
                    div class="modal-content" style="max-width:560px" {
                        h2 { "Link users" }
                        p class="form-hint" style="margin-bottom:12px" {
                            "Pick the same person on each server. Names do not need to match — this mapping tells StateSync who is who."
                        }
                        div class="form-group" {
                            label id="mapServerALabel" { "User on server A" }
                            select id="mapUserA" {}
                        }
                        div class="form-group" {
                            label id="mapServerBLabel" { "User on server B" }
                            select id="mapUserB" {}
                        }
                        div class="modal-actions" style="margin-bottom:16px" {
                            div {}
                            div class="right" {
                                button type="button" class="btn btn-primary" onclick="addLinkedUserMapping()" { "Link these users" }
                            }
                        }
                        div class="form-group" {
                            label { "Current links" }
                            div id="mapLinksList" class="map-links" {}
                        }
                        div class="modal-actions" {
                            div {}
                            div class="right" {
                                button type="button" class="btn" onclick="closeModal('mapUsersModal')" { "Close" }
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
        assert!(html_str.contains("Now playing"));
        assert!(html_str.contains("Media servers"));
        assert!(html_str.contains("Activity log"));
        assert!(html_str.contains("How sync works"));
        assert!(html_str.contains("/favicon.jpg"));
        // No decorative bracket chrome
        assert!(!html_str.contains("[ MAPPED USERS ]"));
    }
}
