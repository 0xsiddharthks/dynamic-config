//! # dynamic-config
//!
//! A Rust library for dynamic configuration that fetches from cloud storage
//! (AWS S3, GCP GCS) and automatically refreshes in the background.
//!
//! ## Overview
//!
//! `dynamic-config` provides a simple way to manage application configuration
//! that can be updated without restarting your service. Configuration is stored
//! in cloud storage (like AWS S3 or GCP GCS) as JSON, and the library handles:
//!
//! - Fetching configuration at startup
//! - Periodic background refresh
//! - Type-safe access via Serde deserialization
//! - Change notifications via subscriptions
//!
//! ## Features
//!
//! - `s3` (default): AWS S3 support
//! - `gcs`: Google Cloud Storage support
//! - `full`: All storage backends
//!
//! ## Quick Start
//!
//! ```no_run
//! use serde::Deserialize;
//! use dynamic_config::{DynamicConfig, source::S3Source};
//! use std::time::Duration;
//!
//! #[derive(Debug, Clone, Deserialize)]
//! struct AppConfig {
//!     a: String,
//!     b: u32,
//!     c: bool,
//! }
//!
//! #[tokio::main]
//! async fn main() -> dynamic_config::error::Result<()> {
//!     // Create S3 source pointing to your config file
//!     let source = S3Source::new("my-config-bucket", "app/config.json").await;
//!
//!     // Build the dynamic config with 1-minute refresh interval
//!     let config: DynamicConfig<AppConfig> = DynamicConfig::builder(source)
//!         .refresh_interval(Duration::from_secs(60))
//!         .build()
//!         .await?;
//!
//!     // Access the current config
//!     let current = config.get().await;
//!     println!("A: {}, B: {}, C: {}", current.a, current.b, current.c);
//!
//!     // Subscribe to changes
//!     let mut subscriber = config.subscribe();
//!     tokio::spawn(async move {
//!         while subscriber.changed().await.is_ok() {
//!             let new_config = subscriber.borrow().clone();
//!             println!("Config updated! Feature enabled: {}", new_config.feature_enabled);
//!         }
//!     });
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod error;
pub mod source;

pub use config::{ConfigHandle, DynamicConfig, DynamicConfigBuilder, DEFAULT_REFRESH_INTERVAL};
pub use error::{DynamicConfigError, Result};
