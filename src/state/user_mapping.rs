use std::collections::HashMap;
use tracing::warn;

fn min_substring_len(a: &str, b: &str) -> usize {
    (a.len().min(b.len()) / 2).max(3)
}

/// Fuzzy substring username matching is off by default because it can map
/// progress to the wrong account (`bob` → `bobby`). Enable only when needed:
/// `STATESYNC_FUZZY_USER_MATCH=true`.
pub fn fuzzy_user_match_enabled() -> bool {
    std::env::var("STATESYNC_FUZZY_USER_MATCH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}

/// Collapse username for soft matching: letters+digits only, lowercased.
/// So `John_Doe` and `johndoe` match without dangerous substring rules.
fn alnum_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Resolve a target-server user id for `source_username`.
///
/// Order: custom mapping groups → exact (case-insensitive) name match →
/// alnum-normalized name match → optional fuzzy substring match.
pub fn find_mapped_user_id(
    source_username: &str,
    target_users: &HashMap<String, String>,
    custom_mappings: &[Vec<String>],
) -> Option<String> {
    let src_lower = source_username.trim().to_lowercase();
    if src_lower.is_empty() {
        return None;
    }

    for group in custom_mappings {
        if group.iter().any(|u| u.to_lowercase() == src_lower) {
            for mapped_name in group {
                let mapped_lower = mapped_name.to_lowercase();
                if mapped_lower != src_lower {
                    if let Some(id) = target_users.get(&mapped_lower) {
                        return Some(id.clone());
                    }
                    // Also try alnum form of mapping entry vs target keys
                    let mapped_alnum = alnum_key(mapped_name);
                    if !mapped_alnum.is_empty() {
                        if let Some((_, id)) = target_users
                            .iter()
                            .find(|(n, _)| alnum_key(n) == mapped_alnum)
                        {
                            return Some(id.clone());
                        }
                    }
                }
            }
        }
    }

    if let Some(id) = target_users.get(&src_lower) {
        return Some(id.clone());
    }

    // Soft match: ignore spaces / underscores / dots / hyphens
    let src_alnum = alnum_key(&src_lower);
    if src_alnum.len() >= 3 {
        let mut alnum_hits: Vec<(&String, &String)> = target_users
            .iter()
            .filter(|(n, _)| alnum_key(n) == src_alnum)
            .collect();
        if alnum_hits.len() == 1 {
            return Some(alnum_hits.remove(0).1.clone());
        }
    }

    if !fuzzy_user_match_enabled() {
        return None;
    }

    let mut candidates: Vec<(&String, &String)> = target_users
        .iter()
        .filter(|(tgt_name, _)| {
            let tgt_lower = tgt_name.to_lowercase();
            let min_len = min_substring_len(&src_lower, &tgt_lower);
            if src_lower.len() < min_len || tgt_lower.len() < min_len {
                return false;
            }
            tgt_lower.contains(&src_lower) || src_lower.contains(&tgt_lower)
        })
        .collect();
    candidates.sort_by(|a, b| {
        let a_diff = (a.0.len() as i64 - src_lower.len() as i64).abs();
        let b_diff = (b.0.len() as i64 - src_lower.len() as i64).abs();
        a_diff.cmp(&b_diff)
    });
    if let Some((name, id)) = candidates.into_iter().next() {
        warn!(
            "fuzzy user match: '{}' → '{}' (set STATESYNC_FUZZY_USER_MATCH=false or use explicit user_mappings)",
            source_username, name
        );
        return Some(id.clone());
    }
    None
}
