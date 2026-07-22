//! Plain-language force-sync storytelling (Emby/Jellyfin libraries only).

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
            "Preview only: count what would change. Nothing is written. Live play sync is paused. We work with Emby/Jellyfin library catalogs (what each app knows about a title) — not your media files.".to_string(),
        )
    } else {
        (
            format!("Force sync started{who}"),
            "Catch-up: make watched, resume, and favorites match between your media apps for linked people. We only talk to Emby/Jellyfin libraries — never open your media files. Live play sync is paused until this finishes.".to_string(),
        )
    }
}

pub fn story_counting(user: &str, source: &str, pair_i: u64, pair_n: u64) -> (String, String) {
    (
        format!("Counting watched titles ({pair_i} of {pair_n})"),
        format!(
            "Person: {user}. Asking {source}’s library how many titles this person already marked watched (and favorited). One combined list from that app — not “all movies first, then all TV.” Next we match those library titles on the other app."
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
        "If the other app differs, count it as would-change (preview does not write)."
    } else {
        "If the other app differs, update watched/resume there."
    };
    (
        format!("Copying watched history ({pair_i} of {pair_n})"),
        format!(
            "Person: {user}. From {source}’s library → to {target}’s library. For each watched title in {source}, find the same title in {target} using shared catalog IDs (IMDb, TMDb, or TVDB) that Emby/Jellyfin already store on that library entry. {write} “Skipped” means we checked and did not need a change (already the same), could not find a shared catalog ID, or {target} has no matching library title."
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
        "Preview only — favorites are not changed."
    } else {
        "Update the heart on the other app only when needed."
    };
    (
        format!("Copying favorites ({pair_i} of {pair_n})"),
        format!(
            "Person: {user}. From {source}’s library → to {target}’s library. Same idea as watched history: match library titles by shared catalog IDs, then {write}"
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
                "Stopped early. Checked {processed} library titles, updated {succeeded}, skipped {skipped}. Live play sync will resume."
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
            "Checked {processed} library titles. Updated (or would update) {succeeded}. Skipped {skipped} (already matched, or no shared catalog ID / no matching title in the other app). Failed {failed}. High skips usually mean both libraries already agree. Live play sync resumes."
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
        assert!(d.contains("library"));
        assert!(!d.to_lowercase().contains("folder"));
    }

    #[test]
    fn played_story_is_about_libraries_not_files() {
        let (h, d) = story_played("bob", "Emby", "Jellyfin", 2, 4, false);
        assert!(h.contains("watched"));
        assert!(d.contains("Emby") && d.contains("Jellyfin"));
        assert!(d.contains("library"));
        assert!(d.contains("Skipped") || d.contains("skipped") || d.contains("“Skipped”"));
        assert!(!d.to_lowercase().contains("folder"));
        assert!(!d.to_lowercase().contains("file name"));
        assert!(!d.to_lowercase().contains("disk"));
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
