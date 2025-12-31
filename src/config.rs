use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use tokio::sync::{watch, RwLock};
use tokio::task::JoinHandle;

use crate::error::Result;
use crate::source::ConfigSource;

pub const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// A dynamic configuration that automatically refreshes from a remote source.
///
/// `DynamicConfig` fetches configuration from a [`ConfigSource`] (like S3 or GCS)
/// at startup and periodically refreshes it in the background.
///
/// # Example
///
/// ```no_run
/// use serde::Deserialize;
/// use dynamic_config::{DynamicConfig, source::S3Source};
/// use std::time::Duration;
///
/// #[derive(Debug, Clone, Deserialize)]
/// struct AppConfig {
///     a: String
///     b: bool,
/// }
///
/// # async fn example() -> dynamic_config::error::Result<()> {
/// let source = S3Source::new("my-bucket", "config.json").await;
///
/// let config: DynamicConfig<AppConfig> = DynamicConfig::builder(source)
///     .refresh_interval(Duration::from_secs(60))
///     .build()
///     .await?;
///
/// // Get current config
/// let current = config.get().await;
/// println!("A: {}, B: {}", current.a, current.b);
///
/// // Subscribe to config changes
/// let mut subscriber = config.subscribe();
/// tokio::spawn(async move {
///     while subscriber.changed().await.is_ok() {
///         let new_config = subscriber.borrow().clone();
///         println!("Config updated! New rate limit: {}", new_config.rate_limits.requests_per_minute);
///     }
/// });
/// # Ok(())
/// # }
/// ```
pub struct DynamicConfig<T> {
    inner: Arc<RwLock<T>>,
    sender: watch::Sender<T>,
    refresh_handle: Option<JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl<T> DynamicConfig<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Gets a clone of the current configuration.
    pub async fn get(&self) -> T {
        self.inner.read().await.clone()
    }

    /// Gets a read guard to the current configuration.
    ///
    /// This is more efficient than `get()` when you only need to read
    /// the config without cloning it.
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, T> {
        self.inner.read().await
    }

    /// Subscribes to configuration changes.
    ///
    /// Returns a receiver that will be notified whenever the config is updated.
    /// The receiver can be cloned to create multiple subscribers.
    pub fn subscribe(&self) -> watch::Receiver<T> {
        self.sender.subscribe()
    }

    /// Shuts down the background refresh task.
    ///
    /// This is automatically called when the DynamicConfig is dropped,
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.refresh_handle.take() {
            handle.abort();
        }
    }
}

impl<T> DynamicConfig<T>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Creates a new DynamicConfig builder with the given source.
    pub fn builder<S: ConfigSource>(source: S) -> DynamicConfigBuilder<T, S> {
        DynamicConfigBuilder::new(source)
    }

    /// Manually triggers a refresh of the configuration.
    ///
    /// This is useful for testing or when you need to force an immediate update.
    /// Returns the new configuration value.
    pub async fn refresh<S: ConfigSource>(&self, source: &S) -> Result<T> {
        let bytes = source.fetch().await?;
        let new_config: T = serde_json::from_slice(&bytes)?;

        {
            let mut guard = self.inner.write().await;
            *guard = new_config.clone();
        }

        // Notify subscribers (ignore if no receivers)
        let _ = self.sender.send(new_config.clone());

        Ok(new_config)
    }
}

impl<T> Drop for DynamicConfig<T> {
    fn drop(&mut self) {
        // Inline shutdown logic to avoid trait bound requirements
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.refresh_handle.take() {
            handle.abort();
        }
    }
}

/// Builder for creating a [`DynamicConfig`].
pub struct DynamicConfigBuilder<T, S: ConfigSource> {
    source: S,
    refresh_interval: Duration,
    _marker: std::marker::PhantomData<T>,
}

impl<T, S> DynamicConfigBuilder<T, S>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
    S: ConfigSource,
{
    /// Creates a new builder with the given source.
    pub fn new(source: S) -> Self {
        Self {
            source,
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            _marker: std::marker::PhantomData,
        }
    }

    /// Sets the interval between automatic config refreshes.
    ///
    /// Default is 5 minutes.
    pub fn refresh_interval(mut self, interval: Duration) -> Self {
        self.refresh_interval = interval;
        self
    }

    /// Builds the DynamicConfig, fetching the initial configuration.
    ///
    /// This will:
    /// 1. Fetch the initial configuration from the source
    /// 2. Start a background task to periodically refresh the config
    ///
    /// Returns an error if the initial fetch fails.
    pub async fn build(self) -> Result<DynamicConfig<T>> {
        tracing::info!(
            source = %self.source.description(),
            refresh_interval = ?self.refresh_interval,
            "Initializing DynamicConfig"
        );

        // Fetch initial config
        let bytes = self.source.fetch().await?;
        let initial_config: T = serde_json::from_slice(&bytes)?;

        tracing::info!(
            source = %self.source.description(),
            "Successfully loaded initial configuration"
        );

        let inner = Arc::new(RwLock::new(initial_config.clone()));
        let (sender, _) = watch::channel(initial_config);
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        // Clone for the background task
        let inner_clone = Arc::clone(&inner);
        let sender_clone = sender.clone();
        let refresh_interval = self.refresh_interval;
        let source = self.source;

        let refresh_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            interval.tick().await; // Skip the first immediate tick

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        tracing::debug!(
                            source = %source.description(),
                            "Refreshing configuration"
                        );

                        match source.fetch().await {
                            Ok(bytes) => {
                                match serde_json::from_slice::<T>(&bytes) {
                                    Ok(new_config) => {
                                        {
                                            let mut guard = inner_clone.write().await;
                                            *guard = new_config.clone();
                                        }

                                        // Notify subscribers
                                        let _ = sender_clone.send(new_config);

                                        tracing::debug!(
                                            source = %source.description(),
                                            "Configuration refreshed successfully"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            source = %source.description(),
                                            error = %e,
                                            "Failed to parse refreshed configuration"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    source = %source.description(),
                                    error = %e,
                                    "Failed to fetch configuration"
                                );
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        tracing::info!(
                            source = %source.description(),
                            "Shutting down configuration refresh task"
                        );
                        break;
                    }
                }
            }
        });

        Ok(DynamicConfig {
            inner,
            sender,
            refresh_handle: Some(refresh_handle),
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// Builds the DynamicConfig without starting background refresh.
    ///
    /// This is useful for testing or when you want to control refresh manually.
    pub async fn build_without_refresh(self) -> Result<DynamicConfig<T>> {
        tracing::info!(
            source = %self.source.description(),
            "Initializing DynamicConfig (no auto-refresh)"
        );

        // Fetch initial config
        let bytes = self.source.fetch().await?;
        let initial_config: T = serde_json::from_slice(&bytes)?;

        let inner = Arc::new(RwLock::new(initial_config.clone()));
        let (sender, _) = watch::channel(initial_config);

        Ok(DynamicConfig {
            inner,
            sender,
            refresh_handle: None,
            shutdown_tx: None,
        })
    }
}

/// A lightweight handle for accessing the current configuration.
///
/// This is useful when you want to pass around access to the config
/// without needing to manage the full `DynamicConfig` lifecycle.
#[derive(Clone)]
pub struct ConfigHandle<T> {
    inner: Arc<RwLock<T>>,
    receiver: watch::Receiver<T>,
}

impl<T> ConfigHandle<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Creates a new handle from a DynamicConfig.
    pub fn new(config: &DynamicConfig<T>) -> Self {
        Self {
            inner: Arc::clone(&config.inner),
            receiver: config.subscribe(),
        }
    }

    /// Gets a clone of the current configuration.
    pub async fn get(&self) -> T {
        self.inner.read().await.clone()
    }

    /// Gets a read guard to the current configuration.
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, T> {
        self.inner.read().await
    }

    /// Returns a receiver for configuration change notifications.
    pub fn subscribe(&self) -> watch::Receiver<T> {
        self.receiver.clone()
    }
}
