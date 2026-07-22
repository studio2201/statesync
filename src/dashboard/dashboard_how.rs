//! "How sync works" dashboard card.
use maud::{Markup, html};

pub fn how_sync_card() -> Markup {
    html! {
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
                                    p { "Mapped users row actions: Force sync (that person only), Clear watched (wipe played flags), Ignore (skip live + mesh force). Confirm carefully." }
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

    }
}
