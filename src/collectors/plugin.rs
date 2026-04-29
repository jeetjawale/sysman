use std::any::Any;
use std::time::Duration;
use anyhow::Result;
use async_trait::async_trait;
use crate::collectors::provider::CommandProvider;
use crate::collectors::Snapshot;

pub enum RefreshStrategy {
    Fixed(Duration),
    OnDemand,
}

pub struct CollectorMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub strategy: RefreshStrategy,
    pub priority: u8,
}

pub struct CollectorOutput {
    pub collector_id: &'static str,
    pub payload: Box<dyn Any + Send>,
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

#[async_trait]
pub trait Collector: Send + Sync {
    fn metadata(&self) -> CollectorMeta;
    
    async fn initialize(&mut self) -> Result<(), CollectorError> { Ok(()) }
    async fn teardown(&mut self) {}

    async fn collect(&self, provider: &dyn CommandProvider, previous: Option<&Snapshot>) 
        -> Result<CollectorOutput, CollectorError>;

    fn view_hint(&self) -> Option<ViewHint> { None }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewHint {
    Gauge,
    Table,
    Sparkline,
}
