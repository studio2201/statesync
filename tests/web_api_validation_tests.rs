//! Path-segment and API-key masking validation (web_api).

use statesync::web_api::mask_api_key;
use statesync::web_api::validation::{valid_item_id, valid_server_name};

#[test]
fn test_item_id_valid_normal() {
    assert!(valid_item_id("item123"));
}
#[test]
fn test_item_id_valid_underscore() {
    assert!(valid_item_id("item_123"));
}
#[test]
fn test_item_id_valid_dash() {
    assert!(valid_item_id("item-123"));
}
#[test]
fn test_item_id_invalid_empty() {
    assert!(!valid_item_id(""));
}
#[test]
fn test_item_id_invalid_too_long() {
    assert!(!valid_item_id(&"a".repeat(65)));
}
#[test]
fn test_item_id_invalid_space() {
    assert!(!valid_item_id("item 123"));
}
#[test]
fn test_item_id_invalid_slash() {
    assert!(!valid_item_id("item/123"));
}
#[test]
fn test_item_id_invalid_dot() {
    assert!(!valid_item_id("item.123"));
}
#[test]
fn test_item_id_numeric() {
    assert!(valid_item_id("123456789"));
}
#[test]
fn test_item_id_dashes() {
    assert!(valid_item_id("a-b-c-d-e"));
}

#[test]
fn test_server_name_valid_normal() {
    assert!(valid_server_name("server1"));
}
#[test]
fn test_server_name_valid_dot() {
    assert!(valid_server_name("server.local"));
}
#[test]
fn test_server_name_valid_dash() {
    assert!(valid_server_name("server-1"));
}
#[test]
fn test_server_name_valid_underscore() {
    assert!(valid_server_name("server_1"));
}
#[test]
fn test_server_name_invalid_empty() {
    assert!(!valid_server_name(""));
}
#[test]
fn test_server_name_invalid_too_long() {
    assert!(!valid_server_name(&"a".repeat(65)));
}
#[test]
fn test_server_name_allows_space() {
    assert!(valid_server_name("server 1"));
}
#[test]
fn test_server_name_allows_at() {
    assert!(valid_server_name("server@home"));
}
#[test]
fn test_server_name_rejects_slash() {
    assert!(!valid_server_name("server/path"));
}
#[test]
fn test_server_name_numeric() {
    assert!(valid_server_name("12345"));
}
#[test]
fn test_server_name_dashes() {
    assert!(valid_server_name("s-e-r-v-e-r"));
}

#[test]
fn test_mask_key_empty() {
    assert_eq!(mask_api_key(""), "");
}
#[test]
fn test_mask_key_short() {
    assert_eq!(mask_api_key("a"), "••••••••");
}
#[test]
fn test_mask_key_exactly_8() {
    assert_eq!(mask_api_key("12345678"), "••••••••");
}
#[test]
fn test_mask_key_exactly_9() {
    assert_eq!(mask_api_key("123456789"), "1234••••••••6789");
}
#[test]
fn test_mask_key_long() {
    assert_eq!(mask_api_key("mysecretkey123456"), "myse••••••••3456");
}
#[test]
fn test_mask_key_spaces() {
    assert_eq!(mask_api_key("key with spaces"), "key ••••••••aces");
}
#[test]
fn test_mask_key_special() {
    assert_eq!(mask_api_key("!@#$%^&*()_+"), "!@#$••••••••()_+");
}
#[test]
fn test_mask_key_numbers() {
    assert_eq!(mask_api_key("000000000"), "0000••••••••0000");
}
#[test]
fn test_mask_key_uppercase() {
    assert_eq!(mask_api_key("ABCDEFGHIJ"), "ABCD••••••••GHIJ");
}
#[test]
fn test_mask_key_mixed() {
    assert_eq!(mask_api_key("aBcDeFgHiJk"), "aBcD••••••••HiJk");
}
#[test]
fn test_mask_key_dots() {
    assert_eq!(mask_api_key(".........."), "....••••••••....");
}
#[test]
fn test_mask_key_slashes() {
    assert_eq!(mask_api_key("//////////"), "////••••••••////");
}
#[test]
fn test_mask_key_all_stars() {
    assert_eq!(mask_api_key("********"), "••••••••");
}
#[test]
fn test_mask_key_long_uuid() {
    assert_eq!(
        mask_api_key("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11"),
        "a0ee••••••••0a11"
    );
}
