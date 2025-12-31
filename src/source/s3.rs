//! AWS S3 configuration source.

use std::future::Future;
use std::pin::Pin;

use aws_sdk_s3::Client;

use crate::error::{DynamicConfigError, Result};
use crate::source::ConfigSource;

/// Configuration source that fetches from AWS S3.
///
/// # Example
///
/// ```no_run
/// use dynamic_config::source::S3Source;
///
/// # async fn example() -> dynamic_config::error::Result<()> {
/// // Using default AWS credentials from environment
/// let source = S3Source::new("my-bucket", "config/app.json").await;
///
/// // Or with a custom endpoint (e.g., for MinIO or LocalStack)
/// let source = S3Source::builder("my-bucket", "config/app.json")
///     .endpoint("http://localhost:9000")
///     .region("us-east-1")
///     .build()
///     .await;
/// # Ok(())
/// # }
/// ```
pub struct S3Source {
    client: Client,
    bucket: String,
    key: String,
}

impl S3Source {
    /// Creates a new S3 source with default AWS configuration
    ///
    /// This uses the default AWS credential chain
    pub async fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        Self {
            client,
            bucket: bucket.into(),
            key: key.into(),
        }
    }

    /// Creates a builder for more customized S3 source configuration
    pub fn builder(bucket: impl Into<String>, key: impl Into<String>) -> S3SourceBuilder {
        S3SourceBuilder::new(bucket, key)
    }
}

impl ConfigSource for S3Source {
    fn fetch(&self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + '_>> {
        Box::pin(async move {
            tracing::debug!(
                bucket = %self.bucket,
                key = %self.key,
                "Fetching config from S3"
            );

            let response = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(&self.key)
                .send()
                .await
                .map_err(|e| DynamicConfigError::FetchError(e.to_string()))?;

            let bytes = response
                .body
                .collect()
                .await
                .map_err(|e| DynamicConfigError::FetchError(e.to_string()))?
                .into_bytes();

            if bytes.is_empty() {
                return Err(DynamicConfigError::EmptyContent);
            }

            tracing::debug!(
                bucket = %self.bucket,
                key = %self.key,
                bytes = bytes.len(),
                "Successfully fetched config from S3"
            );

            Ok(bytes.to_vec())
        })
    }

    fn description(&self) -> String {
        format!("s3://{}/{}", self.bucket, self.key)
    }
}

/// Builder for creating an [`S3Source`] with custom configuration.
pub struct S3SourceBuilder {
    bucket: String,
    key: String,
    endpoint: Option<String>,
    region: Option<String>,
    force_path_style: bool,
}

impl S3SourceBuilder {
    pub fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
            endpoint: None,
            region: None,
            force_path_style: false,
        }
    }

    /// Custom endpoint url
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// AWS Region
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Forces path-style URLs instead of virtual-hosted-style
    ///
    /// Required for local development
    pub fn force_path_style(mut self, force: bool) -> Self {
        self.force_path_style = force;
        self
    }

    pub async fn build(self) -> S3Source {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());

        if let Some(region) = &self.region {
            config_loader = config_loader.region(aws_config::Region::new(region.clone()));
        }

        let sdk_config = config_loader.load().await;

        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

        if let Some(endpoint) = &self.endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint);
        }

        if self.force_path_style {
            s3_config_builder = s3_config_builder.force_path_style(true);
        }

        let client = Client::from_conf(s3_config_builder.build());

        S3Source {
            client,
            bucket: self.bucket,
            key: self.key,
        }
    }
}
