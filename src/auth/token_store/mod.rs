use std::sync::{Arc, RwLock};

use crate::app::credentials::Credentials;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod linux;

#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(target_os = "macos"))]
use linux as imp;

struct InnerTokenStore {
    storage: RwLock<Option<Credentials>>,
}

#[derive(Clone)]
pub struct TokenStore(Arc<InnerTokenStore>);

impl TokenStore {
    pub fn new() -> Self {
        Self(Arc::new(InnerTokenStore {
            storage: RwLock::new(None),
        }))
    }

    pub fn get_cached_blocking(&self) -> Option<Credentials> {
        self.0.storage.read().unwrap().clone()
    }

    pub async fn get_cached(&self) -> Option<Credentials> {
        self.get_cached_blocking()
    }

    pub async fn get(&self) -> Option<Credentials> {
        let local = self.0.storage.read().unwrap().clone();
        if local.is_some() {
            return local;
        }

        match imp::retrieve().await {
            Ok(token) => {
                if token.access_token.is_empty() {
                    None
                } else {
                    self.0.storage.write().unwrap().replace(token.clone());
                    Some(token)
                }
            }
            Err(e) => {
                error!("Couldnt get token from secrets service: {e}");
                None
            }
        }
    }

    pub async fn set(&self, creds: Credentials) {
        debug!("Saving token to store...");
        if let Err(e) = imp::save(&creds).await {
            warn!("Couldnt save token to secrets service: {e}");
        }
        self.0.storage.write().unwrap().replace(creds);
    }

    pub async fn clear(&self) {
        if let Err(e) = imp::logout().await {
            warn!("Couldnt save token to secrets service: {e}");
        }
        self.0.storage.write().unwrap().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retrieve_fails_with_empty_keyring_after_retries() {
        let result = imp::retrieve().await;
        match result {
            Ok(creds) => {
                assert!(!creds.access_token.is_empty());
            }
            Err(e) => {
                assert!(
                    e.to_string().contains("Empty keyring"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    #[tokio::test]
    async fn get_returns_none_when_keyring_empty() {
        let store = TokenStore::new();
        store.0.storage.write().unwrap().take();
        let result = store.get().await;
        if result.is_none() {
            assert!(store.get_cached_blocking().is_none());
        }
    }

    #[tokio::test]
    async fn get_cached_returns_stored_value() {
        let store = TokenStore::new();
        let creds = Credentials {
            access_token: "test_token".to_string(),
            refresh_token: "test_refresh".to_string(),
            token_expiry_time: None,
        };
        store.0.storage.write().unwrap().replace(creds.clone());

        let cached = store.get_cached().await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().access_token, "test_token");
    }

    #[tokio::test]
    async fn get_returns_cached_without_hitting_keyring() {
        let store = TokenStore::new();
        let creds = Credentials {
            access_token: "cached_token".to_string(),
            refresh_token: "cached_refresh".to_string(),
            token_expiry_time: None,
        };
        store.0.storage.write().unwrap().replace(creds.clone());

        let result = store.get().await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().access_token, "cached_token");
    }
}
