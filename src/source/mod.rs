//! Configuration source backends.

use std::future::Future;
use std::pin::Pin;

use crate::error::Result;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "gcs")]
pub mod gcs;

#[cfg(feature = "s3")]
pub use s3::S3Source;

#[cfg(feature = "gcs")]
pub use gcs::GcsSource;

pub trait ConfigSource: Send + Sync + 'static {
    /// Fetches the raw configuration data as bytes.
    fn fetch<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;

    /// Returns a human-readable description of this source for logging.
    fn description(&self) -> String;
}
