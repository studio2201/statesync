#[cfg(test)]
mod tests {
    use super::super::user_mapping::find_mapped_user_id;
    use super::super::cache::ServerCache;
    use std::collections::HashMap;

    #[test]
    fn test_exact_username_match() {
        let mut target_users = HashMap::new();
        target_users.insert("john".to_string(), "id123".to_string());
        let mapped = find_mapped_user_id("JOHN", &target_users, &[]);
        assert_eq!(mapped, Some("id123".to_string()));
    }

    #[test]
    fn test_substring_username_match() {
        let mut target_users = HashMap::new();
        target_users.insert("john".to_string(), "id123".to_string());
        let mapped = find_mapped_user_id("John Doe", &target_users, &[]);
        assert_eq!(mapped, Some("id123".to_string()));

        let mut target_users2 = HashMap::new();
        target_users2.insert("john doe".to_string(), "id456".to_string());
        let mapped2 = find_mapped_user_id("john", &target_users2, &[]);
        assert_eq!(mapped2, Some("id456".to_string()));
    }

    #[test]
    fn test_custom_username_mapping_override() {
        let mut target_users = HashMap::new();
        target_users.insert("john_alt".to_string(), "id999".to_string());
        target_users.insert("john".to_string(), "id123".to_string());
        let custom_mappings = vec![vec!["john_special".to_string(), "john_alt".to_string()]];
        let mapped = find_mapped_user_id("john_special", &target_users, &custom_mappings);
        assert_eq!(mapped, Some("id999".to_string()));
    }

    #[test]
    fn test_username_collision_prevention() {
        let mut target_users = HashMap::new();
        target_users.insert("john smith".to_string(), "id777".to_string());
        let mapped = find_mapped_user_id("john doe", &target_users, &[]);
        assert_eq!(mapped, None);
    }

    #[test]
    fn test_substring_length_guard_rejects_short_lookalikes() {
        let mut target_users = HashMap::new();
        target_users.insert("alice".to_string(), "id_a".to_string());
        target_users.insert("aaron".to_string(), "id_b".to_string());
        let mapped = find_mapped_user_id("a", &target_users, &[]);
        assert_eq!(mapped, None);
    }

    #[test]
    fn test_substring_picks_closest_match() {
        let mut target_users = HashMap::new();
        target_users.insert("alice smith".to_string(), "id_long".to_string());
        target_users.insert("alice".to_string(), "id_short".to_string());
        let mapped = find_mapped_user_id("alice", &target_users, &[]);
        assert_eq!(mapped, Some("id_short".to_string()));
    }

    #[test]
    fn merge_users_preserves_existing_entries() {
        let mut cache = ServerCache {
            name: "emby".to_string(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        };
        cache.users.insert("alice".to_string(), "u1".to_string());
        cache.users.insert("bob".to_string(), "u2".to_string());
        cache.users.insert("carol".to_string(), "u3".to_string());

        let mut fresh = HashMap::new();
        fresh.insert("alice".to_string(), "u1".to_string());
        fresh.insert("dave".to_string(), "u4".to_string());
        cache.merge_users(fresh);

        assert!(cache.users.contains_key("alice"));
        assert!(cache.users.contains_key("bob"));
        assert!(cache.users.contains_key("carol"));
        assert!(cache.users.contains_key("dave"));
        assert_eq!(cache.users.len(), 4);
    }

    #[test]
    fn merge_users_empty_fresh_is_noop() {
        let mut cache = ServerCache {
            name: "emby".to_string(),
            users: HashMap::new(),
            imdb_to_id: HashMap::new(),
            tmdb_to_id: HashMap::new(),
            id_to_providers: HashMap::new(),
        };
        cache.users.insert("alice".to_string(), "u1".to_string());
        cache.merge_users(HashMap::new());
        assert_eq!(cache.users.len(), 1);
        assert!(cache.users.contains_key("alice"));
    }
}
