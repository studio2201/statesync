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
                                "StateSync never moves video. It only copies "
                                strong { "watched" }
                                ", "
                                strong { "resume" }
                                ", and "
                                strong { "favorites" }
                                " between Emby and Jellyfin (and same-type pairs)."
                            }
                            div class="how-pills" {
                                span class="how-pill" { strong { "Live" } " — plays and hearts as they happen" }
                                span class="how-pill" { strong { "Force" } " — history backfill (optional)" }
                                span class="how-pill" { strong { "Actions" } " — per-person Force / Ignore / Clear" }
                            }
                            div class="how-grid" {
                                div class="how-step" {
                                    div class="how-num" { "1" }
                                    div class="how-title" { "Connect" }
                                    p { "Add each server (URL + API key). Type is auto-detected. StateSync opens a live event stream so plays show up here as they happen." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "2" }
                                    div class="how-title" { "Match people" }
                                    p { "Same username matches automatically. Different names → " strong { "Link users" } ". Skip someone with " strong { "Actions → Ignore" } " (or Settings ignore list)." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "3" }
                                    div class="how-title" { "Match titles" }
                                    p { "Titles match by " strong { "Imdb, Tmdb, or Tvdb" } " in Emby/Jellyfin item metadata (API ProviderIds) — " strong { "not" } " folder or file names. Force reuses the in-memory library index first so it does not re-search every title over HTTP. Same title on two libraries syncs when both sides share any of those ids." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "4" }
                                    div class="how-title" { "Live sync" }
                                    p { "Play, pause, finish, or favorite → pushes played / resume / hearts if enabled in " strong { "Settings" } ". Small position wobble under the threshold is ignored." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "5" }
                                    div class="how-title" { "Force sync" }
                                    p { "History backfill. " strong { "Preview force" } " counts with no writes. Live play sync " strong { "pauses" } " until force finishes. Watch the live banner for looked / pushed / skipped." }
                                }
                                div class="how-step" {
                                    div class="how-num" { "6" }
                                    div class="how-title" { "Actions" }
                                    p { "On " strong { "Mapped users" } ", open " strong { "Actions" } " (or click a name first): " strong { "Force" } " this person, " strong { "Ignore" } ", or " strong { "Clear watched" } ". Clear only wipes played flags — not libraries or favorites." }
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
