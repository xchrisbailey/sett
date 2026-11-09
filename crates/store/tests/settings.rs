use std::fs;

use sett_store::{EditorMode, Settings, Space};

#[test]
fn missing_settings_file_gives_defaults_under_home() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::load(&dir.path().join("settings.toml"), dir.path());

    assert_eq!(settings.data_root, dir.path().join("sett"));
    assert_eq!(settings.editor_mode, EditorMode::Block);
    assert_eq!(settings.last_space, Space::Personal);
}

#[test]
fn settings_round_trip_and_bad_fields_fall_back_individually() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/settings.toml");

    let mut settings = Settings::load(&path, dir.path());
    settings.data_root = "/tmp/elsewhere".into();
    settings.editor_mode = EditorMode::Raw;
    settings.last_space = Space::Work;
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path, dir.path()), settings);

    fs::write(
        &path,
        "last_space = \"play\"\neditor_mode = \"raw\"\nunknown = 1\n",
    )
    .unwrap();
    let loaded = Settings::load(&path, dir.path());
    assert_eq!(loaded.editor_mode, EditorMode::Raw);
    assert_eq!(loaded.last_space, Space::Personal);
    assert_eq!(loaded.data_root, dir.path().join("sett"));

    fs::write(&path, "this is = = not toml").unwrap();
    assert_eq!(
        Settings::load(&path, dir.path()).editor_mode,
        EditorMode::Block
    );
}
