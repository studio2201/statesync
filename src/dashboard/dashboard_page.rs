//! Full dashboard HTML template.

use maud::{DOCTYPE, Markup, html};

/// Renders the complete HTML dashboard markup using Maud templates.
pub fn render_dashboard() -> Markup {
    let full_js = super::render_full_js();
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
                style { (maud::PreEscaped(super::styles::css_full())) }
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

                            (super::dashboard_how::how_sync_card())
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
                                p class="form-hint" { "Full browser link or host:port; paths stripped." }
                                p class="form-hint" id="serverTypeHint" { "Type auto-detected on test/save." }
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
                            p class="form-hint" { "Empty = all users. One name per line; linked aliases included." }
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
