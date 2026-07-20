#[cfg(test)]
mod tests {
    use super::super::config::mask_api_key;
    use super::super::validation::{valid_item_id, valid_server_name};

    #[test]
    fn test_mask_api_key() {
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("12345678"), "••••••••");
        assert_eq!(mask_api_key("123456789"), "1234••••••••6789");
        assert_eq!(mask_api_key("my_secret_token_1234"), "my_s••••••••1234");
    }

    #[test]
    fn test_valid_item_id() {
        assert!(valid_item_id("abc123XYZ_-"));
        assert!(!valid_item_id(""));
        assert!(!valid_item_id("../etc/passwd"));
        assert!(!valid_item_id("a b"));
        assert!(!valid_item_id(&"a".repeat(65)));
    }

    #[test]
    fn test_valid_server_name() {
        assert!(valid_server_name("green"));
        assert!(valid_server_name("my-server_01.local"));
        assert!(!valid_server_name(""));
        assert!(!valid_server_name("../etc"));
        assert!(!valid_server_name("name with space"));
    }
}
