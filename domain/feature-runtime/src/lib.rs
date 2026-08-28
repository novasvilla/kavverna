//! Turning features on and off, without any feature knowing it is being managed.
//!
//! A service only knows how to start. Whether it *should* be running is decided here, and
//! being present in the registry is what "running" means. That keeps the decision in one
//! place instead of repeated inside every service.

use feature_catalog::Feature;
use std::collections::BTreeMap;
use strum::IntoEnumIterator;

#[derive(Debug, thiserror::Error)]
#[error("{feature} could not start: {reason}")]
pub struct StartError {
    pub feature: &'static str,
    pub reason: String,
}

impl StartError {
    pub fn new(feature: Feature, reason: impl Into<String>) -> Self {
        Self { feature: feature.id(), reason: reason.into() }
    }
}

pub trait FeatureService {
    fn start(self: Box<Self>) -> Result<Box<dyn RunningFeature>, StartError>;
}

pub trait RunningFeature: Send {
    fn shutdown(self: Box<Self>);
}

pub trait FeatureDemand {
    fn is_wanted(&self, feature: Feature) -> bool;
}

/// Builds a feature's service. The only place that depends on every feature crate.
pub trait FeatureAssembly {
    fn build(&self, feature: Feature) -> Option<Box<dyn FeatureService>>;
}

#[derive(Default)]
pub struct FeatureRegistry {
    running: BTreeMap<Feature, Box<dyn RunningFeature>>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self, feature: Feature) -> bool {
        self.running.contains_key(&feature)
    }

    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Brings the running set in line with demand. Safe to call as often as you like:
    /// features already in the wanted state are left untouched.
    pub fn reconcile(
        &mut self,
        demand: &dyn FeatureDemand,
        assembly: &dyn FeatureAssembly,
    ) -> Vec<StartError> {
        let mut failures = Vec::new();

        for feature in Feature::iter() {
            match (demand.is_wanted(feature), self.running.contains_key(&feature)) {
                (true, false) => {
                    let Some(service) = assembly.build(feature) else {
                        continue;
                    };
                    match service.start() {
                        Ok(running) => {
                            self.running.insert(feature, running);
                            tracing::info!(feature = feature.id(), "started");
                        }
                        Err(err) => {
                            tracing::warn!(feature = feature.id(), %err, "failed to start");
                            failures.push(err);
                        }
                    }
                }
                (false, true) => {
                    if let Some(running) = self.running.remove(&feature) {
                        running.shutdown();
                        tracing::info!(feature = feature.id(), "stopped");
                    }
                }
                _ => {}
            }
        }

        failures
    }

    pub fn shutdown_all(&mut self) {
        for (feature, running) in std::mem::take(&mut self.running) {
            running.shutdown();
            tracing::info!(feature = feature.id(), "stopped");
        }
    }
}
