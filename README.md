# dynamic-config

A Rust library for dynamic configuration that fetches from cloud storage (AWS S3, GCP GCS) and automatically refreshes in the background.

## Overview

`dynamic-config` provides a simple way to manage application configuration that can be updated without restarting your service. Configuration is stored in cloud storage as JSON, and the library handles:

- Fetching configuration at startup
- Periodic background refresh (default: every 5 minutes)
- Type-safe access via Serde deserialization
- Change notifications via subscriptions

## Features

| Feature | Description                  |
| ------- | ---------------------------- |
| `s3`    | AWS S3 support (Default)     |
| `gcs`   | Google Cloud Storage support |
| `full`  | All storage backends         |

## Quick Start

### 1. Define your configuration struct

> NOTE: Nested structs are allowed

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct AppConfig {
    a: String,
    b: u32,
    c: bool,
}
```

### 2. Create a config source and load the config

```rust
use dynamic_config::{DynamicConfig, source::S3Source};
use std::time::Duration;

#[tokio::main]
async fn main() -> dynamic_config::Result<()> {
    // Create S3 source pointing to your config file
    let source = S3Source::builder("my-bucket", "config.json")
        .endpoint("http://localhost:9000") // Replace with S3 Path
        .region("us-east-1")
        .force_path_style(true)
        .build()
        .await;

    // Build the dynamic config with 1-minute refresh interval
    let config: DynamicConfig<AppConfig> = DynamicConfig::builder(source)
        .refresh_interval(Duration::from_secs(60))
        .build()
        .await?;

    // Access the current config
    let current = config.get().await;
    println!("A: {}", current.a);
    println!("B: {}", current.b);

    Ok(())
}
```

### 3. Subscribe to configuration changes

```rust
let mut subscriber = config.subscribe();

tokio::spawn(async move {
    while subscriber.changed().await.is_ok() {
        let new_config = subscriber.borrow().clone();
        println!("Config updated! Feature enabled: {}", new_config.feature_enabled);
    }
});
```

## API Reference

### `DynamicConfig<T>`

The main struct for managing dynamic configuration.

| Method            | Description                                                             |
| ----------------- | ----------------------------------------------------------------------- |
| `builder(source)` | Creates a new builder with the given source                             |
| `get()`           | Returns a clone of the current configuration                            |
| `read()`          | Returns a read guard (more efficient than `get()` for read-only access) |
| `subscribe()`     | Returns a receiver for configuration change notifications               |
| `refresh(source)` | Manually triggers a refresh                                             |
| `shutdown()`      | Stops the background refresh task                                       |

### `DynamicConfigBuilder<T, S>`

Builder for creating a `DynamicConfig`.

| Method                       | Description                                                        |
| ---------------------------- | ------------------------------------------------------------------ |
| `refresh_interval(duration)` | Sets the interval between automatic refreshes (default: 5 minutes) |
| `build()`                    | Builds the config and starts background refresh                    |
| `build_without_refresh()`    | Builds the config without starting background refresh              |

### `ConfigHandle<T>`

A lightweight, cloneable handle for accessing the configuration.

```rust
use dynamic_config::ConfigHandle;

let handle = ConfigHandle::new(&config);
// Pass `handle` to other parts of your application
let current = handle.get().await;
```

## Local dev environment (for running examples)

- setup local S3 container

```bash
cd docker
docker-compose up -d
```

This starts MinIO on `localhost:9000` and automatically creates a bucket with test configuration.

- run example:

```bash
AWS_ACCESS_KEY_ID=minioadmin AWS_SECRET_ACCESS_KEY=minioadmin cargo run --example s3_example
```

- test live updates to config

Open the MinIO Console at http://localhost:9001 (login: `minioadmin`/`minioadmin`), to update the bucket, and check terminal for updates.
