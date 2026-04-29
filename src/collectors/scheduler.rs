use std::sync::Arc;
use tokio::sync::mpsc;
use crate::collectors::plugin::{Collector, CollectorOutput, RefreshStrategy};
use crate::collectors::provider::CommandProvider;
use crate::collectors::Snapshot;

/// Orchestrates the execution of multiple collectors.
pub struct Scheduler {
    provider: Arc<dyn CommandProvider>,
    collectors: Vec<Arc<dyn Collector>>,
}

impl Scheduler {
    pub fn new(provider: Arc<dyn CommandProvider>) -> Self {
        Self {
            provider,
            collectors: Vec::new(),
        }
    }

    /// Registers a new collector.
    pub fn register(&mut self, collector: Box<dyn Collector>) {
        self.collectors.push(Arc::from(collector));
    }

    /// Triggers a refresh of all registered collectors in parallel.
    /// 
    /// In Phase 0, this simple implementation spawns a task for each collector
    /// that is not marked as `OnDemand`.
    pub async fn refresh(&self, previous: Option<&Snapshot>) -> Vec<CollectorOutput> {
        let (tx, mut rx) = mpsc::channel(self.collectors.len().max(1));

        for collector in &self.collectors {
            let meta = collector.metadata();
            
            // For Phase 0, we only skip OnDemand collectors.
            if matches!(meta.strategy, RefreshStrategy::OnDemand) {
                continue;
            }

            let provider = Arc::clone(&self.provider);
            let collector = Arc::clone(collector);
            let tx = tx.clone();
            let prev = previous.cloned();

            tokio::spawn(async move {
                match collector.collect(provider.as_ref(), prev.as_ref()).await {
                    Ok(output) => {
                        let _ = tx.send(output).await;
                    }
                    Err(e) => {
                        // In Phase 0, we log to stderr. Future phases will integrate 
                        // with a more robust error reporting system.
                        eprintln!("Collector {} failed: {}", meta.id, e);
                    }
                }
            });
        }

        // Drop our primary sender so the receiver closes once all tasks finish.
        drop(tx);

        let mut results = Vec::new();
        while let Some(output) = rx.recv().await {
            results.push(output);
        }
        results
    }
}
