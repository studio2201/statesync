//! External provider ids from Emby/Jellyfin item metadata (API ProviderIds).
//! Never from disk paths. Used for matching the same title across servers.

/// Imdb / Tmdb / Tvdb as returned on library items.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderIds {
    pub imdb: String,
    pub tmdb: String,
    pub tvdb: String,
}

impl ProviderIds {
    pub fn is_empty(&self) -> bool {
        self.imdb.is_empty() && self.tmdb.is_empty() && self.tvdb.is_empty()
    }

    /// Stable key for live-sync dedup history (prefixed so id namespaces never collide).
    pub fn history_key(&self) -> Option<String> {
        if !self.imdb.is_empty() {
            Some(format!("imdb:{}", self.imdb))
        } else if !self.tmdb.is_empty() {
            Some(format!("tmdb:{}", self.tmdb))
        } else if !self.tvdb.is_empty() {
            Some(format!("tvdb:{}", self.tvdb))
        } else {
            None
        }
    }

    pub fn display_short(&self) -> String {
        format!(
            "imdb={} tmdb={} tvdb={}",
            if self.imdb.is_empty() {
                "—"
            } else {
                self.imdb.as_str()
            },
            if self.tmdb.is_empty() {
                "—"
            } else {
                self.tmdb.as_str()
            },
            if self.tvdb.is_empty() {
                "—"
            } else {
                self.tvdb.as_str()
            },
        )
    }

    /// Parse Emby/Jellyfin `ProviderIds` object (mixed casing).
    pub fn from_json(providers: Option<&serde_json::Value>) -> Self {
        let Some(providers) = providers else {
            return Self::default();
        };
        Self {
            imdb: first_str(providers, &["Imdb", "imdb", "IMDB"]),
            tmdb: first_str(providers, &["Tmdb", "tmdb", "TMDB", "TmdbId"]),
            tvdb: first_str(providers, &["Tvdb", "tvdb", "TVDB", "TvdbId", "TvDb"]),
        }
    }

    pub fn from_parts(imdb: impl Into<String>, tmdb: impl Into<String>, tvdb: impl Into<String>) -> Self {
        Self {
            imdb: imdb.into(),
            tmdb: tmdb.into(),
            tvdb: tvdb.into(),
        }
    }
}

fn first_str(obj: &serde_json::Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_mixed_case_and_tvdb() {
        let p = ProviderIds::from_json(Some(&json!({
            "Imdb": "tt1",
            "tmdb": "99",
            "Tvdb": "73244"
        })));
        assert_eq!(p.imdb, "tt1");
        assert_eq!(p.tmdb, "99");
        assert_eq!(p.tvdb, "73244");
        assert!(!p.is_empty());
        assert_eq!(p.history_key().as_deref(), Some("imdb:tt1"));
    }

    #[test]
    fn tvdb_only_is_usable() {
        let p = ProviderIds::from_json(Some(&json!({ "tvdb": "121361" })));
        assert!(p.imdb.is_empty() && p.tmdb.is_empty());
        assert!(!p.is_empty());
        assert_eq!(p.history_key().as_deref(), Some("tvdb:121361"));
    }

    #[test]
    fn empty_providers() {
        assert!(ProviderIds::from_json(None).is_empty());
        assert!(ProviderIds::from_json(Some(&json!({}))).is_empty());
    }
}
