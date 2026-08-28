use preferences::Preferences;
use std::os::unix::fs::PermissionsExt;

#[test]
fn values_survive_a_reload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kavverna").join("settings.json");

    let mut prefs = Preferences::load_from(&path);
    prefs.set_bool("keep-awake.allow-display-sleep", false);
    prefs.set_integer("keep-awake.default-minutes", 45);
    prefs.save().expect("save");

    let reloaded = Preferences::load_from(&path);

    assert!(!reloaded.bool("keep-awake.allow-display-sleep", true));
    assert_eq!(reloaded.integer("keep-awake.default-minutes", 0), 45);
}

#[test]
fn an_unset_key_falls_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let prefs = Preferences::load_from(dir.path().join("settings.json"));

    assert!(prefs.bool("never.written", true));
    assert_eq!(prefs.integer("never.written", 7), 7);
}

/// Settings can name machines and habits, so they are not world readable.
#[test]
fn the_file_and_its_directory_are_private() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("kavverna").join("settings.json");

    let mut prefs = Preferences::load_from(&path);
    prefs.set_bool("anything", true);
    prefs.save().expect("save");

    let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(file_mode, 0o600, "settings file is readable by others");
    assert_eq!(dir_mode, 0o700, "settings directory is readable by others");
}

#[test]
fn a_corrupt_file_reads_as_empty_rather_than_failing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{ this is not json").expect("write");

    let prefs = Preferences::load_from(&path);

    assert!(prefs.bool("anything", true));
}

#[test]
fn saving_leaves_no_staging_file_behind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("settings.json");

    let mut prefs = Preferences::load_from(&path);
    prefs.set_bool("anything", true);
    prefs.save().expect("save");

    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();

    assert!(leftovers.is_empty(), "staging file left behind: {leftovers:?}");
}
