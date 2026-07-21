#[cfg(test)]
mod extra_tests {
    use statesync::config::{is_loopback_bind, redacted_url};
    use statesync::web_api::mask_api_key;
    use statesync::state::find_mapped_user_id;
    use statesync::sync_force::{ForceSyncStatus, ForceSyncState, Direction};
    use std::collections::HashMap;

    // --- valid_item_id & valid_server_name tests (16 tests) ---
    use statesync::web_api::validation::{valid_item_id, valid_server_name};

    #[test] fn test_item_id_valid_normal() { assert!(valid_item_id("item123")); }
    #[test] fn test_item_id_valid_underscore() { assert!(valid_item_id("item_123")); }
    #[test] fn test_item_id_valid_dash() { assert!(valid_item_id("item-123")); }
    #[test] fn test_item_id_invalid_empty() { assert!(!valid_item_id("")); }
    #[test] fn test_item_id_invalid_too_long() { assert!(!valid_item_id(&"a".repeat(65))); }
    #[test] fn test_item_id_invalid_space() { assert!(!valid_item_id("item 123")); }
    #[test] fn test_item_id_invalid_slash() { assert!(!valid_item_id("item/123")); }
    #[test] fn test_item_id_invalid_dot() { assert!(!valid_item_id("item.123")); }

    #[test] fn test_server_name_valid_normal() { assert!(valid_server_name("server1")); }
    #[test] fn test_server_name_valid_dot() { assert!(valid_server_name("server.local")); }
    #[test] fn test_server_name_valid_dash() { assert!(valid_server_name("server-1")); }
    #[test] fn test_server_name_valid_underscore() { assert!(valid_server_name("server_1")); }
    #[test] fn test_server_name_invalid_empty() { assert!(!valid_server_name("")); }
    #[test] fn test_server_name_invalid_too_long() { assert!(!valid_server_name(&"a".repeat(65))); }
    #[test] fn test_server_name_invalid_space() { assert!(!valid_server_name("server 1")); }
    #[test] fn test_server_name_invalid_special() { assert!(!valid_server_name("server@")); }

    // --- mask_api_key tests (12 tests) ---
    #[test] fn test_mask_key_empty() { assert_eq!(mask_api_key(""), ""); }
    #[test] fn test_mask_key_short() { assert_eq!(mask_api_key("a"), "••••••••"); }
    #[test] fn test_mask_key_exactly_8() { assert_eq!(mask_api_key("12345678"), "••••••••"); }
    #[test] fn test_mask_key_exactly_9() { assert_eq!(mask_api_key("123456789"), "1234••••••••6789"); }
    #[test] fn test_mask_key_long() { assert_eq!(mask_api_key("mysecretkey123456"), "myse••••••••3456"); }
    #[test] fn test_mask_key_spaces() { assert_eq!(mask_api_key("key with spaces"), "key ••••••••aces"); }
    #[test] fn test_mask_key_special() { assert_eq!(mask_api_key("!@#$%^&*()_+"), "!@#$••••••••()_+"); }
    #[test] fn test_mask_key_numbers() { assert_eq!(mask_api_key("000000000"), "0000••••••••0000"); }
    #[test] fn test_mask_key_uppercase() { assert_eq!(mask_api_key("ABCDEFGHIJ"), "ABCD••••••••GHIJ"); }
    #[test] fn test_mask_key_mixed() { assert_eq!(mask_api_key("aBcDeFgHiJk"), "aBcD••••••••HiJk"); }
    #[test] fn test_mask_key_dots() { assert_eq!(mask_api_key(".........."), "....••••••••...."); }
    #[test] fn test_mask_key_slashes() { assert_eq!(mask_api_key("//////////"), "////••••••••////"); }

    // --- is_loopback_bind tests (12 tests) ---
    #[test] fn test_loopback_ipv4() { assert!(is_loopback_bind("127.0.0.1:4601")); }
    #[test] fn test_loopback_ipv4_no_port() { assert!(is_loopback_bind("127.0.0.1")); }
    #[test] fn test_loopback_localhost() { assert!(is_loopback_bind("localhost:4601")); }
    #[test] fn test_loopback_localhost_no_port() { assert!(is_loopback_bind("localhost")); }
    #[test] fn test_loopback_ipv6() { assert!(is_loopback_bind("[::1]:4601")); }
    #[test] fn test_loopback_ipv6_raw() { assert!(is_loopback_bind("::1")); }
    #[test] fn test_loopback_external_ipv4() { assert!(!is_loopback_bind("192.168.1.1:4601")); }
    #[test] fn test_loopback_external_ipv4_no_port() { assert!(!is_loopback_bind("8.8.8.8")); }
    #[test] fn test_loopback_any_bind() { assert!(!is_loopback_bind("0.0.0.0:4601")); }
    #[test] fn test_loopback_any_bind_ipv6() { assert!(!is_loopback_bind("[::]:4601")); }
    #[test] fn test_loopback_invalid_ipv6_brackets() { assert!(!is_loopback_bind("[::1")); }
    #[test] fn test_loopback_strange_hostname() { assert!(!is_loopback_bind("local-host")); }

    // --- redacted_url tests (12 tests) ---
    #[test] fn test_redact_url_normal_http() { assert_eq!(redacted_url("http://127.0.0.1:8096/path"), "http://127.0.0.1:8096/..."); }
    #[test] fn test_redact_url_normal_https() { assert_eq!(redacted_url("https://media.com/foo/bar"), "https://media.com/..."); }
    #[test] fn test_redact_url_no_path() { assert_eq!(redacted_url("http://localhost"), "http://localhost"); }
    #[test] fn test_redact_url_trailing_slash() { assert_eq!(redacted_url("http://localhost/"), "http://localhost"); }
    #[test] fn test_redact_url_no_scheme() { assert_eq!(redacted_url("localhost:8096/path"), "localhost:8096/path"); }
    #[test] fn test_redact_url_query() { assert_eq!(redacted_url("http://host/path?q=1"), "http://host/..."); }
    #[test] fn test_redact_url_port_slash() { assert_eq!(redacted_url("http://host:80/"), "http://host:80"); }
    #[test] fn test_redact_url_auth() { assert_eq!(redacted_url("http://user:pass@host/path"), "http://user:pass@host/..."); }
    #[test] fn test_redact_url_ip_port() { assert_eq!(redacted_url("http://192.168.1.50:8096"), "http://192.168.1.50:8096"); }
    #[test] fn test_redact_url_subdomain() { assert_eq!(redacted_url("https://sub.domain.com/path"), "https://sub.domain.com/..."); }
    #[test] fn test_redact_url_empty() { assert_eq!(redacted_url(""), ""); }
    #[test] fn test_redact_url_spaces() { assert_eq!(redacted_url("  http://host/path  "), "  http://host/..."); }

    // --- find_mapped_user_id tests (16 tests) ---
    fn prep_users() -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("alice".to_string(), "id_alice".to_string());
        map.insert("bob".to_string(), "id_bob".to_string());
        map.insert("charlie".to_string(), "id_charlie".to_string());
        map
    }

    #[test] fn test_find_user_exact() { assert_eq!(find_mapped_user_id("alice", &prep_users(), &[]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_exact_case() { assert_eq!(find_mapped_user_id("ALICE", &prep_users(), &[]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_mapping_single() { assert_eq!(find_mapped_user_id("ali", &prep_users(), &[vec!["ali".to_string(), "alice".to_string()]]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_mapping_multiple() { assert_eq!(find_mapped_user_id("alias", &prep_users(), &[vec!["alias".to_string(), "bob".to_string(), "charlie".to_string()]]), Some("id_bob".to_string())); }
    #[test] fn test_find_user_substring_contained() { assert_eq!(find_mapped_user_id("alice_smith", &prep_users(), &[]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_substring_contains() { assert_eq!(find_mapped_user_id("ali", &prep_users(), &[]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_substring_short_no_match() { assert_eq!(find_mapped_user_id("al", &prep_users(), &[]), None); }
    #[test] fn test_find_user_not_found() { assert_eq!(find_mapped_user_id("dave", &prep_users(), &[]), None); }
    #[test] fn test_find_user_empty_mappings() { assert_eq!(find_mapped_user_id("alice", &prep_users(), &[]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_mapping_no_target() { assert_eq!(find_mapped_user_id("ali", &prep_users(), &[vec!["ali".to_string(), "dave".to_string()]]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_empty_username() { assert_eq!(find_mapped_user_id("", &prep_users(), &[]), None); }
    #[test] fn test_find_user_space_match() { assert_eq!(find_mapped_user_id("alice ", &prep_users(), &[]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_colliding_substrings() {
        let mut map = HashMap::new();
        map.insert("alice_smith".to_string(), "id1".to_string());
        map.insert("alice_jones".to_string(), "id2".to_string());
        let res = find_mapped_user_id("alice", &map, &[]);
        assert!(res == Some("id1".to_string()) || res == Some("id2".to_string()));
    }
    #[test] fn test_find_user_closest_length() {
        let mut map = HashMap::new();
        map.insert("alice_smith".to_string(), "id1".to_string());
        map.insert("alice".to_string(), "id2".to_string());
        assert_eq!(find_mapped_user_id("alice_s", &map, &[]), Some("id2".to_string())); // "alice" is length diff 2, "alice_smith" is length diff 4
    }
    #[test] fn test_find_user_empty_map() { assert_eq!(find_mapped_user_id("alice", &HashMap::new(), &[]), None); }
    #[test] fn test_find_user_mapping_case_insensitive() { assert_eq!(find_mapped_user_id("ALI", &prep_users(), &[vec!["ali".to_string(), "Alice".to_string()]]), Some("id_alice".to_string())); }

    // --- ForceSyncStatus and Direction/State tests (12 tests) ---
    #[test]
    fn test_force_sync_status_idle() {
        let status = ForceSyncStatus::idle();
        assert_eq!(status.state, ForceSyncState::Idle);
        assert!(status.started_at.is_none());
        assert!(status.finished_at.is_none());
        assert_eq!(status.processed, 0);
    }

    #[test]
    fn test_force_sync_status_default() {
        let status = ForceSyncStatus::default();
        assert_eq!(status.state, ForceSyncState::Idle);
        assert!(status.errors.is_empty());
    }

    #[test]
    fn test_force_sync_state_equality() {
        assert_eq!(ForceSyncState::Idle, ForceSyncState::Idle);
        assert_ne!(ForceSyncState::Running, ForceSyncState::Completed);
    }

    #[test]
    fn test_force_sync_direction_equality() {
        assert_eq!(Direction::Both, Direction::Both);
        assert_ne!(Direction::EmbyToJellyfin, Direction::JellyfinToEmby);
    }

    #[test]
    fn test_force_sync_status_fields() {
        let status = ForceSyncStatus {
            state: ForceSyncState::Completed,
            started_at: Some("start".to_string()),
            finished_at: Some("finish".to_string()),
            direction: Some(Direction::EmbyToJellyfin),
            total_pairs: 10,
            processed: 10,
            succeeded: 8,
            skipped: 1,
            failed: 1,
            current_user: None,
            last_error: Some("err".to_string()),
            errors: vec![],
        };
        assert_eq!(status.state, ForceSyncState::Completed);
        assert_eq!(status.total_pairs, 10);
        assert_eq!(status.succeeded, 8);
        assert_eq!(status.skipped, 1);
        assert_eq!(status.failed, 1);
    }

    #[test] fn test_item_id_numeric() { assert!(valid_item_id("123456789")); }
    #[test] fn test_item_id_dashes() { assert!(valid_item_id("a-b-c-d-e")); }
    #[test] fn test_server_name_numeric() { assert!(valid_server_name("12345")); }
    #[test] fn test_server_name_dashes() { assert!(valid_server_name("s-e-r-v-e-r")); }
    #[test] fn test_mask_key_all_stars() { assert_eq!(mask_api_key("********"), "••••••••"); }
    #[test] fn test_mask_key_long_uuid() { assert_eq!(mask_api_key("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11"), "a0ee••••••••0a11"); }
    #[test] fn test_loopback_ipv6_loopback() { assert!(!is_loopback_bind("[0:0:0:0:0:0:0:1]")); }
    #[test] fn test_loopback_external_host() { assert!(!is_loopback_bind("google.com")); }
    #[test] fn test_redact_url_query_only() { assert_eq!(redacted_url("http://host?"), "http://host?"); }
    #[test] fn test_redact_url_fragment() { assert_eq!(redacted_url("http://host#frag"), "http://host#frag"); }
    #[test] fn test_find_user_mapping_exact_duplicate() { assert_eq!(find_mapped_user_id("alice", &prep_users(), &[vec!["alice".to_string(), "alice".to_string()]]), Some("id_alice".to_string())); }
    #[test] fn test_find_user_case_mismatch_with_mappings() { assert_eq!(find_mapped_user_id("ALICE", &prep_users(), &[vec!["ALICE".to_string(), "bob".to_string()]]), Some("id_bob".to_string())); }
    #[test] fn test_force_sync_state_debug() { assert!(format!("{:?}", ForceSyncState::Running).contains("Running")); }
    #[test] fn test_force_sync_direction_debug() { assert!(format!("{:?}", Direction::Both).contains("Both")); }
    #[test] fn test_force_sync_status_debug() {
        let status = ForceSyncStatus::idle();
        assert!(format!("{:?}", status).contains("state"));
    }

    #[test]
    fn test_file_line_limits_rfc_conventions() {
        use std::fs;
        use std::path::Path;

        fn check_dir(dir: &Path) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if name != "target" && name != ".git" {
                            check_dir(&path);
                        }
                    } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                        let content = fs::read_to_string(&path).expect("failed to read file");
                        let lines = content.lines().count();
                        assert!(
                            lines <= 250,
                            "File {:?} has {} lines, exceeding 250 limit!",
                            path,
                            lines
                        );
                    }
                }
            }
        }
        check_dir(Path::new("."));
    }

    #[test]
    fn test_cargo_rfc_file_tree_structure() {
        use std::path::Path;
        assert!(Path::new("Cargo.toml").exists(), "Cargo.toml must exist");
        assert!(Path::new("src/lib.rs").exists(), "src/lib.rs must exist");
        assert!(Path::new("src/main.rs").exists(), "src/main.rs must exist");
        assert!(Path::new("tests/integration_tests.rs").exists(), "tests/integration_tests.rs must exist");
    }
}
