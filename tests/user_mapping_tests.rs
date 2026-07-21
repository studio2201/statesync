//! Cross-server username → user-id mapping.

use statesync::state::find_mapped_user_id;
use std::collections::HashMap;

fn prep_users() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("alice".to_string(), "id_alice".to_string());
    map.insert("bob".to_string(), "id_bob".to_string());
    map.insert("charlie".to_string(), "id_charlie".to_string());
    map
}

static FUZZY_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_find_user_exact() {
    assert_eq!(
        find_mapped_user_id("alice", &prep_users(), &[]),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_exact_case() {
    assert_eq!(
        find_mapped_user_id("ALICE", &prep_users(), &[]),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_mapping_single() {
    assert_eq!(
        find_mapped_user_id(
            "ali",
            &prep_users(),
            &[vec!["ali".to_string(), "alice".to_string()]]
        ),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_mapping_multiple() {
    assert_eq!(
        find_mapped_user_id(
            "alias",
            &prep_users(),
            &[vec![
                "alias".to_string(),
                "bob".to_string(),
                "charlie".to_string()
            ]]
        ),
        Some("id_bob".to_string())
    );
}
#[test]
fn test_find_user_substring_disabled_by_default() {
    let _g = FUZZY_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
    assert_eq!(find_mapped_user_id("alice_smith", &prep_users(), &[]), None);
    assert_eq!(find_mapped_user_id("ali", &prep_users(), &[]), None);
}
#[test]
fn test_find_user_substring_when_enabled() {
    let _g = FUZZY_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("STATESYNC_FUZZY_USER_MATCH", "true");
    }
    assert_eq!(
        find_mapped_user_id("alice_smith", &prep_users(), &[]),
        Some("id_alice".to_string())
    );
    assert_eq!(
        find_mapped_user_id("ali", &prep_users(), &[]),
        Some("id_alice".to_string())
    );
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
}
#[test]
fn test_find_user_substring_short_no_match() {
    let _g = FUZZY_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("STATESYNC_FUZZY_USER_MATCH", "true");
    }
    assert_eq!(find_mapped_user_id("al", &prep_users(), &[]), None);
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
}
#[test]
fn test_find_user_not_found() {
    assert_eq!(find_mapped_user_id("dave", &prep_users(), &[]), None);
}
#[test]
fn test_find_user_empty_mappings() {
    assert_eq!(
        find_mapped_user_id("alice", &prep_users(), &[]),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_mapping_no_target() {
    let _g = FUZZY_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
    // Custom mapping target missing; without fuzzy, no match.
    assert_eq!(
        find_mapped_user_id(
            "ali",
            &prep_users(),
            &[vec!["ali".to_string(), "dave".to_string()]]
        ),
        None
    );
}
#[test]
fn test_find_user_empty_username() {
    assert_eq!(find_mapped_user_id("", &prep_users(), &[]), None);
}
#[test]
fn test_find_user_space_match() {
    assert_eq!(
        find_mapped_user_id("alice ", &prep_users(), &[]),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_colliding_substrings() {
    let _g = FUZZY_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("STATESYNC_FUZZY_USER_MATCH", "true");
    }
    let mut map = HashMap::new();
    map.insert("alice_smith".to_string(), "id1".to_string());
    map.insert("alice_jones".to_string(), "id2".to_string());
    let res = find_mapped_user_id("alice", &map, &[]);
    assert!(res == Some("id1".to_string()) || res == Some("id2".to_string()));
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
}
#[test]
fn test_find_user_closest_length() {
    let _g = FUZZY_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::set_var("STATESYNC_FUZZY_USER_MATCH", "true");
    }
    let mut map = HashMap::new();
    map.insert("alice_smith".to_string(), "id1".to_string());
    map.insert("alice".to_string(), "id2".to_string());
    assert_eq!(
        find_mapped_user_id("alice_s", &map, &[]),
        Some("id2".to_string())
    ); // "alice" length diff 2; "alice_smith" length diff 4
    unsafe {
        std::env::remove_var("STATESYNC_FUZZY_USER_MATCH");
    }
}
#[test]
fn test_find_user_empty_map() {
    assert_eq!(find_mapped_user_id("alice", &HashMap::new(), &[]), None);
}
#[test]
fn test_find_user_mapping_case_insensitive() {
    assert_eq!(
        find_mapped_user_id(
            "ALI",
            &prep_users(),
            &[vec!["ali".to_string(), "Alice".to_string()]]
        ),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_mapping_exact_duplicate() {
    assert_eq!(
        find_mapped_user_id(
            "alice",
            &prep_users(),
            &[vec!["alice".to_string(), "alice".to_string()]]
        ),
        Some("id_alice".to_string())
    );
}
#[test]
fn test_find_user_case_mismatch_with_mappings() {
    assert_eq!(
        find_mapped_user_id(
            "ALICE",
            &prep_users(),
            &[vec!["ALICE".to_string(), "bob".to_string()]]
        ),
        Some("id_bob".to_string())
    );
}
