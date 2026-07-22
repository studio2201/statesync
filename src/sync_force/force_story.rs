//! First-principles human storytelling for force-sync progress.

use super::ForceSyncStatus;

/// Apply a full story snapshot the WUI can show without guessing.
pub fn apply_story(
    status: &mut ForceSyncStatus,
    phase: &str,
    headline: impl Into<String>,
    detail: impl Into<String>,
    user: Option<&str>,
    source: Option<&str>,
    target: Option<&str>,
    pair_index: u64,
    pair_total: u64,
) {
    status.phase = Some(phase.to_string());
    status.story_headline = Some(headline.into());
    status.story_detail = Some(detail.into());
    status.current_user = user.map(|s| s.to_string());
    status.current_source = source.map(|s| s.to_string());
    status.current_target = target.map(|s| s.to_string());
    status.pair_index = pair_index;
    status.pair_total = pair_total;
}

pub fn story_started(dry_run: bool, only_user: Option<&str>) -> (String, String) {
    let who = only_user
        .map(|u| format!(" for person \"{}\"", u))
        .unwrap_or_else(|| " for every linked person".to_string());
    if dry_run {
        (
            format!("Preview started{who}"),
            "Preview counts what would change. No watched, resume, or favorite data is written. Live play sync is paused until this finishes. Next: count how many titles each person has already watched on each server.".to_string(),
        )
    } else {
        (
            format!("Force sync started{who}"),
            "Goal: make watched history, resume points, and favorites match across your servers for linked people. This is catch-up for the past — not a Movies-then-TV library walk. Live play sync is paused until this finishes. Next: count watched titles on each server.".to_string(),
        )
    }
}

pub fn story_counting(user: &str, source: &str, pair_i: u64, pair_n: u64) -> (String, String) {
    (
        format!("Counting watched titles ({pair_i} of {pair_n})"),
        format!(
            "Person: {user}. Server: {source}. Asking this server how many titles this person has already marked watched (and favorited). Emby/Jellyfin return one combined list — not Movies first, then shows, then music. Folder names and file names are never read. Matching later uses only external ids that the media server already stores on each item (Imdb/Tmdb in API metadata)."
        ),
    )
}

pub fn story_played(
    user: &str,
    source: &str,
    target: &str,
    pair_i: u64,
    pair_n: u64,
    dry_run: bool,
) -> (String, String) {
    let write = if dry_run {
        "If different, count as would-push (preview does not write)."
    } else {
        "If different, write the change on the destination."
    };
    (
        format!("Copying watched history ({pair_i} of {pair_n})"),
        format!(
            "Person: {user}. Route: {source} → {target}. Reading each watched title on {source} (server list order — mixed movies/episodes/etc.). For each title, ask {source} for that item’s stored Imdb/Tmdb provider ids (library metadata — not the folder or file name). Then find the same ids on {target}. {write} \"Skipped\" means we looked and did not write: already the same, the source item had no Imdb/Tmdb in API metadata (it can still look identified in Emby via TVDB or other sources), or the title was not found on {target}. Not a per-library folder scan."
        ),
    )
}

pub fn story_favorites(
    user: &str,
    source: &str,
    target: &str,
    pair_i: u64,
    pair_n: u64,
    dry_run: bool,
) -> (String, String) {
    let write = if dry_run {
        "Preview only — hearts are not changed."
    } else {
        "Write the heart on the destination only when needed."
    };
    (
        format!("Copying favorites ({pair_i} of {pair_n})"),
        format!(
            "Person: {user}. Route: {source} → {target}. Same matching as watched history: Imdb/Tmdb from each item’s server metadata API — never from disk paths. {write} \"Skipped\" still means we checked the title."
        ),
    )
}

pub fn story_finished(
    cancelled: bool,
    dry_run: bool,
    failed: u64,
    processed: u64,
    succeeded: u64,
    skipped: u64,
) -> (String, String) {
    if cancelled {
        return (
            "Force sync cancelled".to_string(),
            format!(
                "Stopped early. Looked at {processed} titles, pushed {succeeded}, skipped {skipped}. Live play sync will resume."
            ),
        );
    }
    let head = if dry_run {
        if failed == 0 {
            "Preview finished (no writes)"
        } else {
            "Preview finished with errors (no writes)"
        }
    } else if failed == 0 {
        "Force sync finished"
    } else {
        "Force sync finished with errors"
    };
    (
        head.to_string(),
        format!(
            "Looked at {processed} titles. Pushed (or would push) {succeeded}. Skipped {skipped} (checked; no write needed, or could not match by Imdb/Tmdb metadata). Failed {failed}. High skips usually mean both servers already agree, or some items have no Imdb/Tmdb in Emby/Jellyfin’s item metadata (even when the UI shows them as identified via other sources). Folder/file names are never required. Live play sync resumes."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_story_names_person_and_server() {
        let (h, d) = story_counting("alice", "Emby", 1, 2);
        assert!(h.contains("Counting"));
        assert!(d.contains("alice"));
        assert!(d.contains("Emby"));
        assert!(d.contains("combined list"));
    }

    #[test]
    fn played_story_names_route() {
        let (h, d) = story_played("bob", "Emby", "Jellyfin", 2, 4, false);
        assert!(h.contains("watched"));
        assert!(d.contains("Emby → Jellyfin"));
        assert!(d.contains("Imdb") || d.contains("Tmdb") || d.contains("metadata"));
        assert!(d.contains("Skipped"));
        assert!(d.contains("not the folder") || d.contains("folder"));
    }

    #[test]
    fn apply_story_sets_route_fields() {
        let mut s = ForceSyncStatus::idle();
        apply_story(
            &mut s,
            "played",
            "headline",
            "detail",
            Some("u"),
            Some("A"),
            Some("B"),
            1,
            3,
        );
        assert_eq!(s.phase.as_deref(), Some("played"));
        assert_eq!(s.current_source.as_deref(), Some("A"));
        assert_eq!(s.current_target.as_deref(), Some("B"));
        assert_eq!(s.pair_index, 1);
        assert_eq!(s.pair_total, 3);
    }
}
