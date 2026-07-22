//! "How sync works" dashboard card.
use maud::{Markup, html};

pub fn how_sync_card() -> Markup {
    html! {
                    div class="card how-sync-card" {
                        div style="display:flex;justify-content:space-between;align-items:center;gap:10px;margin-bottom:10px;flex-wrap:wrap" {
                            div {
                                h2 style="margin:0" { "How sync works" }
                                p class="how-collapsed-hint" id="howSyncCollapsedHint" style="display:none" {
                                    "Live · Force · Actions — expand for the full picture"
                                }
                            }
                            button class="btn" id="toggleHowSyncBtn" onclick="toggleHowSync()" { "Collapse" }
                        }
                        div id="howSyncBody" {
                            p class="how-lead" {
                                "StateSync never touches your media files. It only copies "
                                strong { "watched" }
                                ", "
                                strong { "resume" }
                                ", and "
                                strong { "favorites" }
                                " between "
                                strong { "Emby/Jellyfin libraries" }
                                " (each app’s catalog of titles)."
                            }
                            div class="how-pills" {
                                span class="how-pill" { strong { "Live" } " — plays and hearts as they happen" }
                                span class="how-pill" { strong { "Force" } " — catch up the past (optional)" }
                                span class="how-pill" { strong { "Actions" } " — per-person Force / Ignore / Clear" }
                            }
                            div class="how-grid" {
                                div class="how-step" {
                                    div class="how-num" { "1" }
                                    div class="how-title" { "Connect" }
                                    p { "Add each Emby or Jellyfin server (URL + API key). StateSync listens for plays from that app’s library." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "2" }
                                    div class="how-title" { "Match people" }
                                    p { "Same username matches automatically. Different names → " strong { "Link users" } ". Leave someone out with " strong { "Actions → Ignore" } "." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "3" }
                                    div class="how-title" { "Match titles" }
                                    p { "A title in Emby’s library is the same as a title in Jellyfin’s library when both apps share a catalog ID (IMDb, TMDb, or TVDB) on that library entry. We never look at folders or files." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "4" }
                                    div class="how-title" { "Live sync" }
                                    p { "Play, pause, finish, or favorite in one app → update the same library title in the other (if enabled in " strong { "Settings" } ")." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "5" }
                                    div class="how-title" { "Force sync" }
                                    p { "Catch up older watched history between libraries. " strong { "Preview force" } " counts only. Live sync " strong { "pauses" } " while force runs. The banner shows person, route, and progress." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "6" }
                                    div class="how-title" { "Actions" }
                                    p { "On " strong { "Mapped users" } ": " strong { "Force" } " one person, " strong { "Ignore" } ", or " strong { "Clear watched" } " (played flags only — not the library or favorites)." }
                                }
                            }
                            div class="how-legend" {
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-live" {}
                                    strong { "Live" } " — event stream open; ready for plays"
                                }
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-pending" {}
                                    strong { "Checking access" } " — testing API key"
                                }
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-pending" {}
                                    strong { "Loading data" } " — fetching users / library index"
                                }
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-pending" {}
                                    strong { "Connecting" } " — opening the link"
                                }
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-pending" {}
                                    strong { "Reconnecting" } " — dropped; retrying"
                                }
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-muted" {}
                                    strong { "Offline" } " — no link right now"
                                }
                                span class="how-legend-item" {
                                    span class="how-dot how-dot-failed" {}
                                    strong { "Failed" } " — last attempt errored"
                                }
                            }
                            p class="how-not-synced" {
                                strong { "Not synced:" } " ratings, playlists, libraries, media files."
                            }
                        }
                    }
    }
}
