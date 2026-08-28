use feature_catalog::Feature;
use std::collections::BTreeSet;
use strum::IntoEnumIterator;

/// Guards the persisted contract: a renamed id silently orphans a user's settings, so a
/// change here has to be a deliberate edit of this list.
#[test]
fn ids_match_the_released_set() {
    let released = [
        "clean-url",
        "clipboard-auto-clear",
        "clipboard-history",
        "fan-control",
        "keep-awake",
        "microphone-tools",
        "monitor-alerts",
        "mouse-jiggle",
        "network-monitor",
        "output-switcher",
        "plain-text-paste",
        "system-monitor",
        "volume-mixer",
    ];

    let actual: BTreeSet<_> = Feature::iter().map(Feature::id).collect();
    let expected: BTreeSet<_> = released.into_iter().collect();

    assert_eq!(actual, expected);
}

#[test]
fn ids_are_unique() {
    let ids: Vec<_> = Feature::iter().map(Feature::id).collect();
    let unique: BTreeSet<_> = ids.iter().copied().collect();

    assert_eq!(ids.len(), unique.len(), "duplicate feature id");
}

#[test]
fn every_feature_declares_at_least_one_enable_key() {
    for feature in Feature::iter() {
        assert!(
            !feature.describe().enable_keys.is_empty(),
            "{} has no enable key, so it could never be turned off",
            feature.id()
        );
    }
}
