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
        "clipboard-transform",
        "system-monitor",
        "themes",
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

/// Guards the other half of the contract: marking something Built puts a switch in front of
/// people for code that has to exist. Moving a name up this list is the last step of shipping
/// it, not the first.
#[test]
fn only_what_is_written_is_offered_as_built() {
    let built = [
        "clean-url",
        "clipboard-auto-clear",
        "clipboard-history",
        "clipboard-transform",
        "keep-awake",
        "microphone-tools",
        "mouse-jiggle",
        "output-switcher",
        "system-monitor",
        "themes",
        "volume-mixer",
    ];

    let actual: BTreeSet<_> = Feature::iter().filter(|f| f.is_built()).map(Feature::id).collect();

    assert_eq!(actual, built.into_iter().collect::<BTreeSet<_>>());
}

/// A feature nobody has written cannot be running, whatever a settings file says.
#[test]
fn nothing_planned_is_installed_by_default() {
    for feature in Feature::iter().filter(|f| !f.is_built()) {
        assert!(
            !feature.installed_by_default(),
            "{} is not built, so it must not arrive switched on",
            feature.id()
        );
    }
}
