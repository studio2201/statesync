use std::collections::HashMap;

fn min_substring_len(a: &str, b: &str) -> usize {
    (a.len().min(b.len()) / 2).max(3)
}

pub fn find_mapped_user_id(
    source_username: &str,
    target_users: &HashMap<String, String>,
    custom_mappings: &[Vec<String>],
) -> Option<String> {
    let src_lower = source_username.to_lowercase();

    for group in custom_mappings {
        if group.iter().any(|u| u.to_lowercase() == src_lower) {
            for mapped_name in group {
                let mapped_lower = mapped_name.to_lowercase();
                if mapped_lower != src_lower {
                    if let Some(id) = target_users.get(&mapped_lower) {
                        return Some(id.clone());
                    }
                }
            }
        }
    }

    if let Some(id) = target_users.get(&src_lower) {
        return Some(id.clone());
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
    if let Some((_, id)) = candidates.into_iter().next() {
        return Some(id.clone());
    }
    None
}
