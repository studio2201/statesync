use maud::{DOCTYPE, Markup, html};

pub fn render_dashboard() -> Markup {
    html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    meta charset="UTF-8";
                    meta name="viewport" content="width=device-width, initial-scale=1.0";
                    meta name="theme-color" content="#03060f";
                    title { "StateSync" }
                    link rel="manifest" href="/manifest.json";
                    link rel="apple-touch-icon" href="/icon.svg";
                    link rel="shortcut icon" href="/favicon.jpg" type="image/jpeg";
                    link href="https://fonts.googleapis.com/css2?family=Share+Tech+Mono&display=swap" rel="stylesheet";
                    style {
                        (maud::PreEscaped(include_str!("index.css")))
                    }
                }
                body {
                    div class="container" {
                        h1 {
                            span { "StateSync" }
                            div style="display: flex; gap: 10px; align-items: center;" {
                                button class="btn" id="refreshUsersBtn" onclick="refreshUsers()" { "[ REFRESH USERS ]" }
                                button class="btn btn-accent" id="forceSyncBtn" onclick="forceSync()" { "[ FORCE SYNC ]" }
                                button class="btn btn-accent" onclick="openSettingsModal()" { "[ SETTINGS ]" }
                                button class="btn" onclick="openServerModal(-1)" { "[ + ADD MEDIA SERVER ]" }
                            }
                        }
                        div id="lastFullSyncBanner" style="margin-bottom:20px;padding:10px 14px;border:1px solid rgba(255,255,255,0.1);background:rgba(0,0,0,0.2);font-size:12px;color:var(--text);display:flex;justify-content:space-between;align-items:center" {}
                        div id="forceSyncLive" style="margin-bottom:20px;padding:12px 14px;border:1px solid var(--border);background:rgba(0,240,255,0.06);font-size:12px;display:none" {
                            div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px" {
                                div { "FULL SYNC IN PROGRESS" }
                                div id="fsProgressText" style="color:var(--border)" {}
                            }
                            progress id="fsProgressBar" value="0" max="100" style="width:100%;height:8px;-webkit-appearance:none;appearance:none" {}
                            div id="fsCurrentUser" style="margin-top:6px;font-size:11px;color:var(--text);opacity:0.8" {}
                            div style="margin-top:8px;text-align:right" {
                                button class="btn btn-danger" id="fsCancelBtn" onclick="cancelForceSync()" { "CANCEL" }
                            }
                        }
                        div class="row-grid" {
                            div class="card" style="display:flex; flex-direction:column; height: 100%; box-sizing: border-box;" {
                                h2 { "[ MAPPED USERS ]" }
                                div id="syncedUsers" style="display: flex; flex-direction: column; gap: 8px; flex-grow: 1;" {}
                                div id="forceSyncStatus" style="margin-top:10px;font-size:11px;color:var(--text);opacity:0.7" {}
                            }
                            div style="display: flex; flex-direction: column; gap: 25px;" {
                                div class="card" {
                                    h2 { "[ ACTIVE STREAMS ]" }
                                    div id="activeSessions" style="display: flex; flex-direction: column; gap: 10px;" {
                                        div style="color: var(--accent)" { "NO ACTIVE STREAMS DETECTED" }
                                    }
                                }
                                div class="card" {
                                    h2 { "[ MEDIA SERVERS ]" }
                                    div class="server-list" id="serverList" {}
                                }
                                div class="card" {
                                    h2 style="display:flex;justify-content:space-between;align-items:center" {
                                        span { "[ TERMINAL LOG FEED ]" }
                                        button class="btn-small" id="toggleLogsBtn" onclick="toggleLogs()" { "[ EXPAND ]" }
                                    }
                                    div class="log-feed" id="syncLogs" style="display:none" {}
                                }
                            }
                        }
                        div id="versionFooter" class="version-footer" {}
                    }
                    div class="modal" id="serverModal" {
                        div class="modal-content" {
                            h2 id="modalTitle" { "[ CONFIGURE MEDIA SERVER ]" }
                            form id="serverForm" {
                                div class="form-group" {
                                    label { "SERVER TYPE" }
                                    div class="radio-row" {
                                        button type="button" class="btn-radio btn-radio-jellyfin active" id="btnJellyfin" onclick="pickType('jellyfin')" { "JELLYFIN" }
                                        button type="button" class="btn-radio btn-radio-emby" id="btnEmby" onclick="pickType('emby')" { "EMBY" }
                                    }
                                    input type="hidden" id="serverType" value="jellyfin" {}
                                }
                                div class="form-group" {
                                    label { "SERVER ADDRESS" }
                                    div style="display:flex;gap:8px;align-items:center" {
                                        input type="url" id="serverUrl" placeholder="http://emby.local:8096" required="" style="flex:1";
                                        button type="button" class="btn" id="autoNameBtn" onclick="autoFetchServerName()" { "↻ AUTO" }
                                    }
                                }
                                div class="form-group" {
                                    label { "DISPLAY NAME" }
                                    input type="text" id="serverName" required="" placeholder="(auto-filled from server when ↻ AUTO clicked)" {}
                                }
                                div class="form-group" {
                                    label { "API KEY" }
                                    input type="password" id="serverKey" required="" {}
                                }
                                div class="form-group" {
                                    label { "SYNC DIRECTION" }
                                    div class="radio-row" {
                                        button type="button" class="btn-radio active" data-dir="both" onclick="pickDirection('both')" { "BIDIRECTIONAL" }
                                        button type="button" class="btn-radio" data-dir="send" onclick="pickDirection('send')" { "SEND ONLY" }
                                        button type="button" class="btn-radio" data-dir="receive" onclick="pickDirection('receive')" { "RECEIVE ONLY" }
                                    }
                                    input type="hidden" id="serverDirection" value="both" {}
                                }
                                div style="display:flex;justify-content:space-between;margin-top:20px;" {
                                    button type="submit" class="btn" { "[ SAVE ]" }
                                    button type="button" class="btn btn-accent" onclick="testConnection()" { "[ TEST LINK ]" }
                                    button type="button" class="btn btn-danger" onclick="closeModal('serverModal')" { "[ ABORT ]" }
                                }
                            }
                        }
                    }
                    div class="modal" id="settingsModal" {
                        div class="modal-content" {
                            h2 { "[ GLOBAL SETTINGS ]" }
                            div class="form-group" {
                                label { "UI THEME" }
                                select id="themeSelector" onchange="setTheme(this.value)" {
                                    option value="cyberpunk" { "CYBERPUNK" }
                                    option value="matrix" { "MATRIX" }
                                    option value="outrun" { "SYNTHWAVE" }
                                    option value="crimson" { "CRIMSON" }
                                    option value="solarized" { "SOLARIZED" }
                                    option value="nordic" { "NORDIC" }
                                    option value="mono" { "MONOCHROME" }
                                    option value="military" { "MILITARY" }
                                    option value="royal" { "ROYAL" }
                                }
                            }
                            div class="form-group" {
                                label { "SYNC WINDOW THRESHOLD (SECONDS)" }
                                input type="number" id="syncThreshold" min="1" value="5" {}
                                div style="font-size:11px;color:var(--text);opacity:0.6;margin-top:2px" { "Default: 5. Lower = more aggressive dedup, higher = more sync events." }
                            }
                            div class="form-group" {
                                label { "MANUAL USER MAPPINGS (COMMA-SEPARATED, ONE GROUP PER LINE)" }
                                textarea id="cfgUserMappings" rows="3" style="background:#03060f;border:1px solid var(--accent);color:#fff;font-family:monospace;width:100%;box-sizing:border-box;padding:8px;" placeholder="john doe, john\njane, jane_doe" {}
                            }
                            div style="display:flex;gap:12px;margin-top:20px;" {
                                button class="btn" onclick="saveSettings()" { "[ SAVE ]" }
                                button class="btn btn-danger" onclick="closeModal('settingsModal')" { "[ ABORT ]" }
                            }
                        }
                    }
                    div class="toast" id="toast" {}
                    div class="modal" id="authModal" style="display:none" {
                        div class="modal-content" {
                            h2 { "[ AUTHENTICATION REQUIRED ]" }
                            p style="color: var(--text); font-size: 12px; margin-bottom: 12px;" {
                                "This dashboard is protected. Enter the bearer token configured on the server."
                            }
                            div class="form-group" {
                                label { "BEARER TOKEN" }
                                input type="password" id="authToken" autocomplete="off" {}
                            }
                            div style="display:flex;justify-content:flex-end;margin-top:20px;gap:12px" {
                                button class="btn btn-accent" id="authSubmitBtn" { "[ UNLOCK ]" }
                            }
                        }
                    }
                    script {
                        (maud::PreEscaped(include_str!("index.js")))
                    }
                }
            }
        }
}
