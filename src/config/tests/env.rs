use super::TEST_LOCK;
use crate::config::Config;

#[test]
fn test_config_load_from_env_flat() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_SERVER_0_URL", "https://env-server:8096");
        std::env::set_var("STATESYNC_SERVER_0_API_KEY", "env_key");
        std::env::set_var("STATESYNC_SERVER_0_NAME", "Env Server");
        std::env::set_var("STATESYNC_SERVER_0_TYPE", "emby");
        std::env::set_var("STATESYNC_SERVER_0_DIRECTION", "send");
        std::env::set_var("STATESYNC_SERVER_0_INSECURE", "false");
        std::env::set_var("STATESYNC_SYNC_THRESHOLD_SECONDS", "42");
    }

    let loaded = Config::load().unwrap();
    assert_eq!(loaded.servers.len(), 1);
    assert_eq!(loaded.servers[0].url, "https://env-server:8096");
    assert_eq!(loaded.servers[0].name, "Env Server");
    assert!(loaded.servers[0].is_emby);
    assert_eq!(loaded.servers[0].sync_direction, "send");
    assert!(!loaded.servers[0].allow_insecure_http);
    assert_eq!(loaded.sync_threshold_seconds, 42);

    unsafe {
        std::env::remove_var("STATESYNC_SERVER_0_URL");
        std::env::remove_var("STATESYNC_SERVER_0_API_KEY");
        std::env::remove_var("STATESYNC_SERVER_0_NAME");
        std::env::remove_var("STATESYNC_SERVER_0_TYPE");
        std::env::remove_var("STATESYNC_SERVER_0_DIRECTION");
        std::env::remove_var("STATESYNC_SERVER_0_INSECURE");
        std::env::remove_var("STATESYNC_SYNC_THRESHOLD_SECONDS");
    }
}

#[test]
fn test_config_load_from_env_fallback_two_servers() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_EMBY_URL", "https://emby-fallback:8096");
        std::env::set_var("STATESYNC_EMBY_API_KEY", "emby_key");
        std::env::set_var("STATESYNC_JELLYFIN_URL", "https://jf-fallback:8096");
        std::env::set_var("STATESYNC_JELLYFIN_API_KEY", "jf_key");
        std::env::set_var("STATESYNC_ALLOW_INSECURE_HTTP", "false");
    }

    let loaded = Config::load().unwrap();
    assert_eq!(loaded.servers.len(), 2);
    assert_eq!(loaded.servers[0].name, "Emby");
    assert_eq!(loaded.servers[0].url, "https://emby-fallback:8096");
    assert!(loaded.servers[0].is_emby);
    assert!(!loaded.servers[0].allow_insecure_http);
    assert_eq!(loaded.servers[1].name, "Jellyfin");
    assert_eq!(loaded.servers[1].url, "https://jf-fallback:8096");
    assert!(!loaded.servers[1].is_emby);
    assert!(!loaded.servers[1].allow_insecure_http);

    unsafe {
        std::env::remove_var("STATESYNC_EMBY_URL");
        std::env::remove_var("STATESYNC_EMBY_API_KEY");
        std::env::remove_var("STATESYNC_JELLYFIN_URL");
        std::env::remove_var("STATESYNC_JELLYFIN_API_KEY");
        std::env::remove_var("STATESYNC_ALLOW_INSECURE_HTTP");
    }
}

#[test]
fn test_config_load_invalid_insecure_flag() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_SERVER_0_URL", "https://s0");
        std::env::set_var("STATESYNC_SERVER_0_API_KEY", "key");
        std::env::set_var("STATESYNC_SERVER_0_INSECURE", "0");
    }
    let loaded = Config::load().unwrap();
    assert!(!loaded.servers[0].allow_insecure_http);

    unsafe {
        std::env::remove_var("STATESYNC_SERVER_0_URL");
        std::env::remove_var("STATESYNC_SERVER_0_API_KEY");
        std::env::remove_var("STATESYNC_SERVER_0_INSECURE");
    }
}

#[test]
fn test_config_load_invalid_threshold() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_SERVER_0_URL", "https://s0");
        std::env::set_var("STATESYNC_SERVER_0_API_KEY", "key");
        std::env::set_var("STATESYNC_SYNC_THRESHOLD_SECONDS", "invalid");
    }
    let loaded = Config::load().unwrap();
    assert_eq!(loaded.sync_threshold_seconds, 5);

    unsafe {
        std::env::remove_var("STATESYNC_SERVER_0_URL");
        std::env::remove_var("STATESYNC_SERVER_0_API_KEY");
        std::env::remove_var("STATESYNC_SYNC_THRESHOLD_SECONDS");
    }
}

#[test]
fn test_config_load_missing_name_defaults() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_SERVER_0_URL", "https://s0");
        std::env::set_var("STATESYNC_SERVER_0_API_KEY", "key");
    }
    let loaded = Config::load().unwrap();
    assert_eq!(loaded.servers[0].name, "Server 0");

    unsafe {
        std::env::remove_var("STATESYNC_SERVER_0_URL");
        std::env::remove_var("STATESYNC_SERVER_0_API_KEY");
    }
}

#[test]
fn test_config_load_two_servers_fallback_allow_insecure() {
    let _guard = TEST_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("STATESYNC_EMBY_URL", "https://e1");
        std::env::set_var("STATESYNC_EMBY_API_KEY", "ek");
        std::env::set_var("STATESYNC_JELLYFIN_URL", "https://j1");
        std::env::set_var("STATESYNC_JELLYFIN_API_KEY", "jk");
        std::env::set_var("STATESYNC_ALLOW_INSECURE_HTTP", "true");
    }
    let loaded = Config::load().unwrap();
    assert!(loaded.servers[0].allow_insecure_http);
    assert!(loaded.servers[1].allow_insecure_http);

    unsafe {
        std::env::remove_var("STATESYNC_EMBY_URL");
        std::env::remove_var("STATESYNC_EMBY_API_KEY");
        std::env::remove_var("STATESYNC_JELLYFIN_URL");
        std::env::remove_var("STATESYNC_JELLYFIN_API_KEY");
        std::env::remove_var("STATESYNC_ALLOW_INSECURE_HTTP");
    }
}
