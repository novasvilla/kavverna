use feature_catalog::Feature;
use feature_runtime::{
    FeatureAssembly, FeatureDemand, FeatureRegistry, FeatureService, RunningFeature, StartError,
};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct Wanted(BTreeSet<Feature>);

impl FeatureDemand for Wanted {
    fn is_wanted(&self, feature: Feature) -> bool {
        self.0.contains(&feature)
    }
}

struct CountingAssembly {
    builds: Rc<RefCell<Vec<Feature>>>,
    stops: Arc<AtomicUsize>,
}

struct Service {
    stops: Arc<AtomicUsize>,
}

struct Handle {
    stops: Arc<AtomicUsize>,
}

impl FeatureService for Service {
    fn start(self: Box<Self>) -> Result<Box<dyn RunningFeature>, StartError> {
        Ok(Box::new(Handle { stops: self.stops }))
    }
}

impl RunningFeature for Handle {
    fn shutdown(self: Box<Self>) {
        self.stops.fetch_add(1, Ordering::SeqCst);
    }
}

impl FeatureAssembly for CountingAssembly {
    fn build(&self, feature: Feature) -> Option<Box<dyn FeatureService>> {
        self.builds.borrow_mut().push(feature);
        Some(Box::new(Service { stops: Arc::clone(&self.stops) }))
    }
}

fn assembly() -> (CountingAssembly, Rc<RefCell<Vec<Feature>>>, Arc<AtomicUsize>) {
    let builds = Rc::new(RefCell::new(Vec::new()));
    let stops = Arc::new(AtomicUsize::new(0));
    let a = CountingAssembly { builds: Rc::clone(&builds), stops: Arc::clone(&stops) };
    (a, builds, stops)
}

/// The guarantee the whole crate boundary exists to provide.
#[test]
fn an_unwanted_feature_is_never_built() {
    let (assembly, builds, _) = assembly();
    let demand = Wanted([Feature::KeepAwake].into_iter().collect());
    let mut registry = FeatureRegistry::new();

    registry.reconcile(&demand, &assembly);

    assert_eq!(*builds.borrow(), vec![Feature::KeepAwake]);
    assert_eq!(registry.running_count(), 1);
}

#[test]
fn reconciling_twice_does_not_restart_anything() {
    let (assembly, builds, stops) = assembly();
    let demand = Wanted([Feature::KeepAwake, Feature::CleanUrl].into_iter().collect());
    let mut registry = FeatureRegistry::new();

    registry.reconcile(&demand, &assembly);
    registry.reconcile(&demand, &assembly);
    registry.reconcile(&demand, &assembly);

    assert_eq!(builds.borrow().len(), 2, "a running feature was rebuilt");
    assert_eq!(stops.load(Ordering::SeqCst), 0, "a wanted feature was stopped");
}

#[test]
fn dropping_demand_shuts_the_feature_down() {
    let (assembly, _, stops) = assembly();
    let mut registry = FeatureRegistry::new();

    registry.reconcile(&Wanted([Feature::KeepAwake].into_iter().collect()), &assembly);
    assert!(registry.is_running(Feature::KeepAwake));

    registry.reconcile(&Wanted::default(), &assembly);

    assert!(!registry.is_running(Feature::KeepAwake));
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}

#[test]
fn a_feature_that_fails_to_start_is_reported_and_left_stopped() {
    struct Failing;
    impl FeatureService for Failing {
        fn start(self: Box<Self>) -> Result<Box<dyn RunningFeature>, StartError> {
            Err(StartError::new(Feature::FanControl, "no permission"))
        }
    }
    struct Assembly;
    impl FeatureAssembly for Assembly {
        fn build(&self, _: Feature) -> Option<Box<dyn FeatureService>> {
            Some(Box::new(Failing))
        }
    }

    let mut registry = FeatureRegistry::new();
    let failures =
        registry.reconcile(&Wanted([Feature::FanControl].into_iter().collect()), &Assembly);

    assert_eq!(failures.len(), 1);
    assert!(!registry.is_running(Feature::FanControl));
}
