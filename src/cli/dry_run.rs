use std::collections::HashSet;
use std::time::Duration;
use statesync::client::MediaClient;
use statesync::config::Config;
use statesync::state::init_server_cache;

pub async fn trigger_reload() -> anyhow::Result<()> {
    println!("Sending reload signal to active statesync service...");
    let url = std::env::var("STATESYNC_RELOAD_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4601/api/reload".to_string());
    let token = std::env::var("STATESYNC_WEB_AUTH").ok();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut req = client.post(&url);
    if let Some(t) = token {
        if let Some(b) = t.strip_prefix("bearer:") {
            req = req.bearer_auth(b);
        }
    }
    match req.send().await {
        Ok(resp) => {
            if resp.status() == reqwest::StatusCode::OK {
                println!("✓ Reload signal successfully sent. Active service is reloading config.");
                Ok(())
            } else {
                let err_text = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                println!("✗ Active service returned error: {}", err_text);
                std::process::exit(1);
            }
        }
        Err(e) => {
            println!("✗ Failed to connect to active statesync service: {}", e);
            println!("Make sure the statesync background container/service is running.");
            std::process::exit(1);
        }
    }
}

pub async fn validate_config() -> anyhow::Result<()> {
    println!("=== CONFIGURATION VALIDATION ===");
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("✗ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };
    println!("✓ Config file parsed successfully.");
    println!("Found {} configured server(s).", config.servers.len());
    println!(
        "Sync threshold: {} seconds.\n",
        config.sync_threshold_seconds
    );

    let mut all_ok = true;
    for s in &config.servers {
        println!("Checking connection to '{}' ({})...", s.name, s.url);
        let client = MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby);
        match init_server_cache(&s.name, &client).await {
            Ok(cache) => {
                println!(
                    "  ✓ Connected successfully! Loaded {} users, {} media items.",
                    cache.users.len(),
                    cache.id_to_providers.len()
                );
            }
            Err(e) => {
                println!("  ✗ Connection failed: {}", e);
                all_ok = false;
            }
        }
    }

    if all_ok {
        println!("\n✓ All checks passed! Configuration is valid.");
        Ok(())
    } else {
        println!("\n✗ Some checks failed. Please check your network and API keys.");
        std::process::exit(1);
    }
}

pub async fn dry_run() -> anyhow::Result<()> {
    println!("=== DRY RUN ===");
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("✗ Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };
    println!("Loaded {} server(s).", config.servers.len());
    let mut caches = Vec::new();
    for s in &config.servers {
        println!("Initializing cache for '{}'...", s.name);
        let client = MediaClient::new(s.url.clone(), s.api_key.clone(), s.is_emby);
        match init_server_cache(&s.name, &client).await {
            Ok(c) => caches.push(c),
            Err(e) => {
                println!("  ✗ '{}' failed: {}", s.name, e);
                std::process::exit(1);
            }
        }
    }
    let mut seen: HashSet<(usize, String)> = HashSet::new();
    let mut ambiguous = 0u32;
    for (idx, cache) in caches.iter().enumerate() {
        for username in cache.users.keys() {
            let key = (idx, username.clone());
            if seen.contains(&key) {
                continue;
            }
            for (other_idx, other_cache) in caches.iter().enumerate() {
                if other_idx == idx {
                    continue;
                }
                let matched = statesync::state::find_mapped_user_id(
                    username,
                    &other_cache.users,
                    &config.user_mappings,
                );
                if let Some(_id) = matched {
                    seen.insert(key.clone());
                    seen.insert((other_idx, _id));
                }
            }
        }
    }
    for c in &caches {
        if c.users.is_empty() {
            println!("  ! '{}' has no users", c.name);
            ambiguous += 1;
        }
    }
    if ambiguous > 0 {
        println!("\n✗ {} problem(s) detected.", ambiguous);
        std::process::exit(1);
    }
    println!("\n✓ Dry run complete; no problems detected.");
    Ok(())
}
