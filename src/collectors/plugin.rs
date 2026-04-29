use std::any::Any;
use std::time::Duration;
use anyhow::Result;
use async_trait::async_trait;
use crate::collectors::provider::CommandProvider;
use crate::collectors::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStrategy {
    Fixed(Duration),
    OnDemand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectorMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub strategy: RefreshStrategy,
    pub priority: u8,
}

pub struct CollectorOutput {
    pub collector_id: &'static str,
    pub payload: Box<dyn Any + Send + Sync>,
}

#[derive(Debug, thiserror::Error)]
pub enum CollectorError {
    #[error("Dependency missing: {0}")]
    DependencyMissing(String),
    #[error("Command failed: {cmd}, stderr: {stderr}")]
    CommandFailed { cmd: String, stderr: String },
    #[error("Parse failure: {0}")]
    ParseFailure(String),
    #[error("Timed out after {0:?}")]
    Timeout(Duration),
}

/// Core trait for data collection components.
#[async_trait]
pub trait Collector: Send + Sync {
    /// Returns the static metadata for this collector.
    fn metadata(&self) -> CollectorMeta;
    
    /// Optional initialization logic called when the collector is first registered.
    async fn initialize(&mut self) -> Result<(), CollectorError> { Ok(()) }

    /// Optional cleanup logic called before the collector is dropped.
    async fn teardown(&mut self) {}

    /// Main collection logic. Invoked based on the defined `RefreshStrategy`.
    async fn collect(&self, provider: &dyn CommandProvider, previous: Option<&Snapshot>) 
        -> Result<CollectorOutput, CollectorError>;

    /// Provides a hint to the UI on how to best render this collector's data.
    fn view_hint(&self) -> Option<ViewHint> { None }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewHint {
    Gauge,
    Table,
    Sparkline,
}
