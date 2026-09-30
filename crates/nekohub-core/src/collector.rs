use async_trait::async_trait;
use thiserror::Error;

use crate::{HostTarget, RawHostSample, StorageSnapshot};

/// Failure from a single collection attempt.
#[derive(Debug, Clone, Error)]
pub enum CollectError {
    #[error("connection timed out")]
    Timeout,
    #[error("transport failed: {0}")]
    Transport(String),
    #[error("remote probe failed: {0}")]
    Remote(String),
    #[error("invalid probe response: {0}")]
    Protocol(String),
}

/// Port implemented by any source capable of collecting a Linux host sample.
#[async_trait]
pub trait Collector: Send + Sync {
    async fn collect(&self, host: &HostTarget) -> Result<RawHostSample, CollectError>;

    async fn collect_storage(
        &self,
        _host: &HostTarget,
        _path: &str,
    ) -> Result<StorageSnapshot, CollectError> {
        Err(CollectError::Remote(
            "storage inventory is not supported by this collector".into(),
        ))
    }
}
