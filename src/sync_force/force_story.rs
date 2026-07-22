//! Dense force-sync facts (Emby/Jellyfin libraries only). No fluff.

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
        .map(|u| format!(" for \"{u}\""))
        .unwrap_or_else(|| " (all linked people)".to_string());
    if dry_run {
        (
            format!("Preview started{who}"),
            "Mode: preview (no writes). Scope: Emby/Jellyfin libraries only. Live sync: paused."
                .to_string(),
        )
    } else {
        (
            format!("Force started{who}"),
            "Mode: write. Goal: match watched / resume / favorites across libraries. Live sync: paused."
                .to_string(),
        )
    }
}

pub fn story_counting(user: &str, source: &str, pair_i: u64, pair_n: u64) -> (String, String) {
    (
        format!("Counting watched ({pair_i}/{pair_n})"),
        format!("Step: count. Person: {user}. Server: {source}. Action: ask library how many watched (+ favorites)."),
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
    let mode = if dry_run { "preview" } else { "write" };
    (
        format!("Watched history ({pair_i}/{pair_n})"),
        format!(
            "Step: watched. Person: {user}. Route: {source} → {target}. Match: catalog ID (IMDb/TMDb/TVDB). Mode: {mode}. No change = already same, or no shared ID / not in {target}."
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
    let mode = if dry_run { "preview" } else { "write" };
    (
        format!("Favorites ({pair_i}/{pair_n})"),
        format!(
            "Step: favorites. Person: {user}. Route: {source} → {target}. Match: catalog ID. Mode: {mode}."
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
            "Force cancelled".to_string(),
            format!(
                "Result: cancelled. Checked {processed}. Updated {succeeded}. No change {skipped}. Failed {failed}. Live sync: resumes."
            ),
        );
    }
    let head = if dry_run {
        if failed == 0 {
            "Preview finished"
        } else {
            "Preview finished (failures)"
        }
    } else if failed == 0 {
        "Force finished"
    } else {
        "Force finished (failures)"
    };
    (
        head.to_string(),
        format!(
            "Result: done. Checked {processed}. Updated {succeeded}. No change {skipped}. Failed {failed}. Live sync: resumes."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counting_story_is_dense() {
        let (h, d) = story_counting("alice", "Emby", 1, 2);
        assert!(h.contains("Counting"));
        assert!(d.contains("alice") && d.contains("Emby"));
        assert!(d.contains("Step:"));
        assert!(d.len() < 160);
    }

    #[test]
    fn played_story_names_route_not_files() {
        let (h, d) = story_played("bob", "Emby", "Jellyfin", 2, 4, false);
        assert!(h.contains("Watched"));
        assert!(d.contains("Emby → Jellyfin"));
        assert!(d.contains("No change"));
        assert!(!d.to_lowercase().contains("folder"));
        assert!(!d.to_lowercase().contains("skip"));
        assert!(d.len() < 220);
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
        assert_eq!(s.current_source.as_deref(), Some("A"));
        assert_eq!(s.pair_index, 1);
    }
}
