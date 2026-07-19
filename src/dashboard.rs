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
                                select id="themeSelector" class="btn" style="background:#000;padding:7px 10px;" onchange="setTheme(this.value)" {
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
                                button class="btn btn-accent" onclick="openSettingsModal()" { "[ SETTINGS ]" }
                                button class="btn" onclick="openServerModal(-1)" { "[ + ADD MODULE ]" }
                            }
                        }
                        div class="row-grid" {
    div class="card" {
                            div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:15px;flex-wrap:wrap;gap:8px" {
                                h2 style="margin:0" { "[ MAPPED USERS ]" }
                                div style="display:flex;gap:6px" {
                                    button class="btn-small" id="refreshUsersBtn" onclick="refreshUsers()" { "REFRESH" }
                                    button class="btn-small btn-accent-small" id="forceSyncBtn" onclick="forceSync()" { "FORCE SYNC" }
                                }
                            }
                            div id="syncedUsers" style="display: flex; flex-direction: column; gap: 8px;" {}
                            div id="forceSyncStatus" style="margin-top:10px;font-size:11px;color:var(--text);opacity:0.7" {}
                        }
                            div class="card" {
                                h2 { "[ STREAM MONITOR ]" }
                                div id="activeSessions" style="display: flex; flex-direction: column; gap: 10px;" {
                                    div style="color: var(--accent)" { "NO ACTIVE STREAMS DETECTED" }
                                }
                            }
                        }
                        div class="card" {
                            h2 { "[ TRANSCEIVER MODULES ]" }
                            div class="server-list" id="serverList" {}
                        }
                        div class="card" {
                            h2 { "[ TERMINAL LOG FEED ]" }
                            div class="log-feed" id="syncLogs" {}
                        }
                        div id="versionFooter" class="version-footer" {}
                    }
                    div class="modal" id="serverModal" {
                        div class="modal-content" {
                            h2 id="modalTitle" { "[ CONFIGURE MODULE ]" }
                            form id="serverForm" {
                                div class="form-group" {
                                    label { "MODULE TYPE" }
                                    select id="serverType" {
                                        option value="jellyfin" { "JELLYFIN" }
                                        option value="emby" { "EMBY" }
                                    }
                                }
                                div class="form-group" {
                                    label { "IDENT NAME" }
                                    input type="text" id="serverName" required="" {}
                                }
                                div class="form-group" {
                                    label { "TRANSCEIVER IP:PORT" }
                                    input type="url" id="serverUrl" placeholder="http://emby.local:8096" required="" {}
                                }
                                div class="form-group" {
                                    label { "ACCESS KEY (API)" }
                                    input type="password" id="serverKey" required="" {}
                                }
                                div class="form-group" {
                                    label { "SYNC DIRECTION" }
                                    select id="serverDirection" {
                                        option value="both" { "BI-DIRECTIONAL" }
                                        option value="send" { "SEND ONLY" }
                                        option value="receive" { "RECEIVE ONLY" }
                                    }
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
