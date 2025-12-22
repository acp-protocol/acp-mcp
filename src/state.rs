//! @acp:module "Application State"
//! @acp:summary "Shared state for ACP daemon including loaded schemas"
//! @acp:domain daemon
//! @acp:layer service
//!
//! Manages the loaded ACP schemas (config, cache, vars) and provides
//! thread-safe access for request handlers.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use acp::cache::Cache;
use acp::config::Config;
use acp::vars::VarsFile;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Shared application state for the daemon
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    /// Project root directory
    project_root: PathBuf,
    /// Loaded ACP config
    config: RwLock<Config>,
    /// Loaded ACP cache
    cache: RwLock<Cache>,
    /// Loaded ACP vars
    vars: RwLock<Option<VarsFile>>,
}

impl AppState {
    /// Load ACP state from project directory
    pub async fn load(project_root: &Path) -> anyhow::Result<Self> {
        // Load config
        let config_path = project_root.join(".acp.config.json");
        let config = if config_path.exists() {
            let content = tokio::fs::read_to_string(&config_path).await?;
            serde_json::from_str(&content)?
        } else {
            info!("No .acp.config.json found, using defaults");
            Config::default()
        };

        // Load cache
        let cache_path = project_root.join(".acp").join("acp.cache.json");
        let cache = if cache_path.exists() {
            let content = tokio::fs::read_to_string(&cache_path).await?;
            serde_json::from_str(&content)?
        } else {
            return Err(anyhow::anyhow!(
                "No cache found at {}. Run 'acp index' first.",
                cache_path.display()
            ));
        };

        // Load vars (optional)
        let vars_path = project_root.join(".acp").join("acp.vars.json");
        let vars = if vars_path.exists() {
            match tokio::fs::read_to_string(&vars_path).await {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!("Failed to parse vars: {}", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Failed to read vars: {}", e);
                    None
                }
            }
        } else {
            info!("No vars file found at {}", vars_path.display());
            None
        };

        Ok(Self {
            inner: Arc::new(AppStateInner {
                project_root: project_root.to_path_buf(),
                config: RwLock::new(config),
                cache: RwLock::new(cache),
                vars: RwLock::new(vars),
            }),
        })
    }

    /// Create AppState for testing with in-memory cache
    #[cfg(test)]
    pub fn for_testing(cache: Cache, vars: Option<VarsFile>) -> Self {
        Self {
            inner: Arc::new(AppStateInner {
                project_root: std::path::PathBuf::from("."),
                config: RwLock::new(Config::default()),
                cache: RwLock::new(cache),
                vars: RwLock::new(vars),
            }),
        }
    }

    /// Get project root
    #[allow(dead_code)]
    pub fn project_root(&self) -> &Path {
        &self.inner.project_root
    }

    /// Get read access to config
    pub async fn config(&self) -> tokio::sync::RwLockReadGuard<'_, Config> {
        self.inner.config.read().await
    }

    /// Get read access to cache (async)
    pub async fn cache_async(&self) -> tokio::sync::RwLockReadGuard<'_, Cache> {
        self.inner.cache.read().await
    }

    /// Get read access to vars
    pub async fn vars(&self) -> tokio::sync::RwLockReadGuard<'_, Option<VarsFile>> {
        self.inner.vars.read().await
    }

    /// Reload cache from disk (for hot-reload, Phase 4)
    #[allow(dead_code)]
    pub async fn reload_cache(&self) -> anyhow::Result<()> {
        let cache_path = self.inner.project_root.join(".acp").join("acp.cache.json");
        let content = tokio::fs::read_to_string(&cache_path).await?;
        let cache: Cache = serde_json::from_str(&content)?;

        let mut write_guard = self.inner.cache.write().await;
        *write_guard = cache;

        info!("Cache reloaded from disk");
        Ok(())
    }

    /// Reload vars from disk (for hot-reload, Phase 4)
    #[allow(dead_code)]
    pub async fn reload_vars(&self) -> anyhow::Result<()> {
        let vars_path = self.inner.project_root.join(".acp").join("acp.vars.json");
        if vars_path.exists() {
            let content = tokio::fs::read_to_string(&vars_path).await?;
            let vars: VarsFile = serde_json::from_str(&content)?;

            let mut write_guard = self.inner.vars.write().await;
            *write_guard = Some(vars);

            info!("Vars reloaded from disk");
        }
        Ok(())
    }
}
