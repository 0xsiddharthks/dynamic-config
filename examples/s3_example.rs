use std::time::Duration;

use dynamic_config::{
    source::{ConfigSource, S3Source},
    DynamicConfig,
};
use serde::Deserialize;

/// Example application configuration.
///
/// This struct represents the Type for the JSON configuration, stored in S3/GCS
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub version: String,
    pub nested_attributes: NestedAttributes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NestedAttributes {
    pub value: bool,
}

#[tokio::main]
async fn main() -> dynamic_config::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("dynamic_config=debug".parse().unwrap()),
        )
        .init();

    println!("=== Dynamic Config S3 Example ===\n");

    // Create an S3 source pointing to MinIO
    let source = S3Source::builder("config-bucket", "app-config.json")
        .endpoint("http://localhost:9000")
        .region("us-east-1")
        .force_path_style(true)
        .build()
        .await;

    println!("Created S3 source: {}\n", source.description());

    // Build the dynamic config with a 30-second refresh interval
    // (shorter for demo purposes; in production you might use 5 minutes)
    let config: DynamicConfig<AppConfig> = DynamicConfig::builder(source)
        .refresh_interval(Duration::from_secs(30))
        .build()
        .await?;

    println!("Successfully loaded initial configuration!\n");

    // Display the current configuration
    let current = config.get().await;
    print_config(&current);

    // Set up a subscriber to watch for config changes
    let mut subscriber = config.subscribe();

    println!("\n--- Watching for config changes (Ctrl+C to exit) ---");
    println!("Try modifying the config in MinIO to see live updates!\n");

    // Keep the main task alive and watch for changes
    loop {
        tokio::select! {
            result = subscriber.changed() => {
                match result {
                    Ok(()) => {
                        println!("\n=== Configuration Updated! ===\n");
                        let new_config = subscriber.borrow().clone();
                        print_config(&new_config);
                    }
                    Err(_) => {
                        println!("Config channel closed, exiting...");
                        break;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nReceived Ctrl+C, shutting down...");
                break;
            }
        }
    }

    Ok(())
}

fn print_config(config: &AppConfig) {
    println!("App Name: {}", config.name);
    println!("Version: {}", config.version);
    println!();
    println!("Value: {}", config.nested_attributes.value);
}
