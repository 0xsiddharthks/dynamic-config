//! Google Cloud Storage configuration source.

use std::future::Future;
use std::pin::Pin;

use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;

use crate::error::{DynamicConfigError, Result};
use crate::source::ConfigSource;

/// Configuration source that fetches from Google Cloud Storage.
///
/// # Example
///
/// ```no_run
/// use dynamic_config::source::GcsSource;
///
/// # async fn example() -> dynamic_config::error::Result<()> {
/// // Using default GCP credentials from environment
/// let source = GcsSource::new("my-bucket", "config/app.json").await?;
/// # Ok(())
/// # }
/// ```
pub struct GcsSource {
    client: Client,
    bucket: String,
    object: String,
}

impl GcsSource {
    /// Creates a new GCS source with default GCP configuration.
    ///
    /// This uses the default GCP credential chain
    pub async fn new(
        bucket: impl Into<String>,
        object: impl Into<String>,
    ) -> std::result::Result<Self, DynamicConfigError> {
        let config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| DynamicConfigError::FetchError(e.to_string()))?;
        let client = Client::new(config);

        Ok(Self {
            client,
            bucket: bucket.into(),
            object: object.into(),
        })
    }

    /// Creates a GCS source with anonymous access (for public buckets)
    pub fn anonymous(bucket: impl Into<String>, object: impl Into<String>) -> Self {
        let config = ClientConfig::default().anonymous();
        let client = Client::new(config);

        Self {
            client,
            bucket: bucket.into(),
            object: object.into(),
        }
    }

    /// Creates a GCS source with a pre-configured client
    pub fn with_client(
        client: Client,
        bucket: impl Into<String>,
        object: impl Into<String>,
    ) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            object: object.into(),
        }
    }
}

impl ConfigSource for GcsSource {
    fn fetch(&self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async move {
            tracing::debug!(
                bucket = %self.bucket,
                object = %self.object,
                "Fetching config from GCS"
            );

            let request = GetObjectRequest {
                bucket: self.bucket.clone(),
                object: self.object.clone(),
                ..Default::default()
            };

            let contents = self
                .client
                .download_object(&request, &Range::default())
                .await
                .map_err(|e| DynamicConfigError::FetchError(e.to_string()))?;

            if contents.is_empty() {
                return Err(DynamicConfigError::EmptyContent);
            }

            tracing::debug!(
                bucket = %self.bucket,
                object = %self.object,
                bytes = contents.len(),
                "Successfully fetched config from GCS"
            );

            Ok(contents)
        })
    }

    fn description(&self) -> String {
        format!("gs://{}/{}", self.bucket, self.object)
    }
}
