use crate::db::Database;
use crate::library_backup::{
    backup_content_digest, parse_and_normalize, read_bounded, reconcile_persisted_sources,
    serialize_bounded, validate_export_destination, verify_preview_digest, write_atomically,
    BackupClientSettings, BackupError, MAX_BACKUP_BYTES, MAX_RELATIONS_PER_WALLPAPER,
    MAX_TITLE_CHARS,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn valid_backup() -> Value {
    json!({
        "app": "PureWall",
        "schemaVersion": 1,
        "exportedAt": "2026-07-29T00:00:00Z",
        "sources": [],
        "wallpapers": [],
        "tags": [],
        "collections": [],
        "settings": {}
    })
}

fn parse(value: &Value) -> Result<crate::library_backup::NormalizedBackup, BackupError> {
    parse_and_normalize(&serde_json::to_vec(value).expect("backup fixture should serialize"))
}

fn expect_error_code(value: &Value, code: &str) {
    let error = parse(value).expect_err("backup fixture should be rejected");
    assert_eq!(error.code(), code);
}

fn unique_temp_path(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "purewall-backup-{test_name}-{}-{nanos}",
        std::process::id()
    ))
}

fn remove_fixture(path: &Path) {
    if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn accepts_the_exact_purewall_v1_envelope() {
    let parsed = parse(&valid_backup()).expect("valid v1 backup should parse");

    assert_eq!(parsed.document.app, "PureWall");
    assert_eq!(parsed.document.schema_version, 1);
    assert!(parsed.document.wallpapers.is_empty());
}

#[test]
fn accepts_only_the_actual_workspace_mode_vocabulary() {
    for workspace_mode in ["workbench", "quiet"] {
        let mut backup = valid_backup();
        backup["settings"] = json!({ "workspaceMode": workspace_mode });
        parse(&backup).expect("current PureWall workspace modes should parse");
    }

    for workspace_mode in ["library", "focus"] {
        let mut backup = valid_backup();
        backup["settings"] = json!({ "workspaceMode": workspace_mode });
        let error = parse(&backup).expect_err("stale workspace modes should be rejected");
        assert_eq!(error.code(), "BACKUP_INVALID_DATA");
    }
}

#[test]
fn rejects_invalid_app_and_unsupported_schema_with_stable_codes() {
    let mut wrong_app = valid_backup();
    wrong_app["app"] = json!("OtherWall");
    expect_error_code(&wrong_app, "BACKUP_INVALID_APP");

    let mut future_schema = valid_backup();
    future_schema["schemaVersion"] = json!(2);
    expect_error_code(&future_schema, "BACKUP_UNSUPPORTED_SCHEMA");
}

#[test]
fn rejects_input_larger_than_the_bounded_backup_limit() {
    let bytes = vec![b' '; MAX_BACKUP_BYTES as usize + 1];
    let error = parse_and_normalize(&bytes).expect_err("oversized backup should be rejected");

    assert_eq!(error.code(), "BACKUP_TOO_LARGE");
}

#[test]
fn canonicalizes_existing_paths_through_the_filesystem_boundary() {
    let root = unique_temp_path("canonical-path");
    remove_fixture(&root);
    std::fs::create_dir_all(root.join("nested")).expect("fixture directory should exist");
    let wallpaper = root.join("nested").join("wall.jpg");
    std::fs::write(&wallpaper, b"fixture").expect("fixture wallpaper should exist");
    let canonical_root = root
        .canonicalize()
        .expect("fixture root should canonicalize");
    let canonical_wallpaper = wallpaper
        .canonicalize()
        .expect("fixture wallpaper should canonicalize");

    let mut backup = valid_backup();
    backup["sources"] = json!([{
        "path": root.join(".").to_string_lossy(),
        "source": "mounted"
    }]);
    backup["wallpapers"] = json!([{
        "path": root.join(".").join("nested").join("wall.jpg").to_string_lossy(),
        "source": "mounted",
        "hash": "hash-a",
        "displayTitle": null,
        "rating": 1,
        "hidden": false,
        "width": 1920,
        "height": 1080,
        "fileSize": 7,
        "tags": [],
        "collections": []
    }]);

    let parsed = parse(&backup).expect("existing paths should normalize");

    assert_eq!(
        parsed.document.sources[0].path,
        canonical_root.to_string_lossy()
    );
    assert_eq!(
        parsed.document.wallpapers[0].path,
        canonical_wallpaper.to_string_lossy()
    );

    remove_fixture(&root);
}

#[test]
fn rejects_relative_paths_invalid_ratings_and_oversized_titles() {
    let mut relative_source = valid_backup();
    relative_source["sources"] = json!([{
        "path": "relative/walls",
        "source": "mounted"
    }]);
    expect_error_code(&relative_source, "BACKUP_INVALID_DATA");

    let mut invalid_rating = valid_backup();
    invalid_rating["wallpapers"] = json!([{
        "path": absolute_missing_path("invalid-rating.jpg"),
        "source": "mounted",
        "hash": "",
        "displayTitle": null,
        "rating": 2,
        "hidden": false,
        "width": 0,
        "height": 0,
        "fileSize": 0,
        "tags": [],
        "collections": []
    }]);
    expect_error_code(&invalid_rating, "BACKUP_INVALID_DATA");

    let mut oversized_title = invalid_rating.clone();
    oversized_title["wallpapers"][0]["rating"] = json!(0);
    oversized_title["wallpapers"][0]["displayTitle"] = json!("x".repeat(MAX_TITLE_CHARS + 1));
    expect_error_code(&oversized_title, "BACKUP_INVALID_DATA");
}

#[test]
fn rejects_duplicate_identities_and_undefined_relations() {
    let path = absolute_missing_path("duplicate.jpg");
    let wallpaper = json!({
        "path": path,
        "source": "mounted",
        "hash": "",
        "displayTitle": null,
        "rating": 0,
        "hidden": false,
        "width": 0,
        "height": 0,
        "fileSize": 0,
        "tags": [],
        "collections": []
    });
    let mut duplicate_wallpapers = valid_backup();
    duplicate_wallpapers["wallpapers"] = json!([wallpaper.clone(), wallpaper.clone()]);
    expect_error_code(&duplicate_wallpapers, "BACKUP_INVALID_DATA");

    let mut duplicate_tags = valid_backup();
    duplicate_tags["tags"] = json!([
        { "name": "Nature", "color": "#008800" },
        { "name": " Nature ", "color": "#008800" }
    ]);
    expect_error_code(&duplicate_tags, "BACKUP_INVALID_DATA");

    let mut undefined_relation = valid_backup();
    let mut wallpaper_with_missing_tag = wallpaper;
    wallpaper_with_missing_tag["tags"] = json!(["Missing"]);
    undefined_relation["wallpapers"] = json!([wallpaper_with_missing_tag]);
    expect_error_code(&undefined_relation, "BACKUP_INVALID_DATA");
}

#[test]
fn rejects_wallpaper_relation_sets_above_the_fixed_limit() {
    let names = (0..=MAX_RELATIONS_PER_WALLPAPER)
        .map(|index| format!("tag-{index}"))
        .collect::<Vec<_>>();
    let definitions = names
        .iter()
        .map(|name| json!({ "name": name, "color": "#0a84ff" }))
        .collect::<Vec<_>>();
    let mut backup = valid_backup();
    backup["tags"] = json!(definitions);
    backup["wallpapers"] = json!([{
        "path": absolute_missing_path("relations.jpg"),
        "source": "mounted",
        "hash": "",
        "displayTitle": null,
        "rating": 0,
        "hidden": false,
        "width": 0,
        "height": 0,
        "fileSize": 0,
        "tags": names,
        "collections": []
    }]);

    expect_error_code(&backup, "BACKUP_INVALID_DATA");
}

#[test]
fn exports_a_consistent_sorted_snapshot_with_only_allowlisted_metadata() {
    let root = unique_temp_path("snapshot");
    remove_fixture(&root);
    fs::create_dir_all(root.join("source-z")).expect("source z should exist");
    fs::create_dir_all(root.join("source-a")).expect("source a should exist");
    let database = Database::new(root.join("purewall.db")).expect("test database should open");
    let source_z = root.join("source-z").to_string_lossy().to_string();
    let source_a = root.join("source-a").to_string_lossy().to_string();
    let wallpaper_z = root.join("z-wall.jpg").to_string_lossy().to_string();
    let wallpaper_a = root.join("a-wall.jpg").to_string_lossy().to_string();

    database
        .upsert_watched_folder(&source_z, "mounted")
        .expect("source z should persist");
    database
        .upsert_watched_folder(&source_a, "imported-folder")
        .expect("source a should persist");
    database
        .upsert_wallpaper(&wallpaper_z, "hash-z", "mounted", 2560, 1440, 42)
        .expect("wallpaper z should persist");
    database
        .upsert_wallpaper(&wallpaper_a, "hash-a", "imported-file", 1920, 1080, 24)
        .expect("wallpaper a should persist");
    database
        .set_rating(&wallpaper_z, 1)
        .expect("rating should persist");
    database
        .set_blacklisted(&wallpaper_z, true)
        .expect("hidden state should persist");
    database
        .set_wallpaper_display_title(&wallpaper_z, "  Zed title  ")
        .expect("display title should persist");
    database
        .record_play(&wallpaper_z)
        .expect("play history fixture should persist");

    let tag_z = database
        .create_tag("Zebra", "#222222")
        .expect("tag z should persist");
    let tag_a = database
        .create_tag("Alpha", "#111111")
        .expect("tag a should persist");
    database
        .assign_tag(&wallpaper_z, tag_z.id)
        .expect("tag z relation should persist");
    database
        .assign_tag(&wallpaper_z, tag_a.id)
        .expect("tag a relation should persist");
    let collection_z = database
        .create_collection("Travel", "#444444")
        .expect("collection z should persist");
    let collection_a = database
        .create_collection("Archive", "#333333")
        .expect("collection a should persist");
    database
        .assign_collection(&wallpaper_z, collection_z.id)
        .expect("collection z relation should persist");
    database
        .assign_collection(&wallpaper_z, collection_a.id)
        .expect("collection a relation should persist");

    database
        .set_setting("rotation_secs", "900")
        .expect("rotation setting should persist");
    database
        .set_setting("display_mode", "span")
        .expect("display setting should persist");
    database
        .set_setting("focus_mode_enabled", "true")
        .expect("focus setting should persist");
    database
        .set_setting("paused", "false")
        .expect("pause setting should persist");
    database
        .set_setting("maintenance_secret", "do-not-export")
        .expect("unapproved setting fixture should persist");

    let document = database
        .export_backup_snapshot(
            BackupClientSettings {
                theme: Some("dark".to_string()),
                workspace_mode: Some("workbench".to_string()),
            },
            "2026-07-29T12:00:00Z".to_string(),
        )
        .expect("snapshot should export");

    assert_eq!(document.exported_at, "2026-07-29T12:00:00Z");
    assert_eq!(
        document
            .sources
            .iter()
            .map(|source| source.path.as_str())
            .collect::<Vec<_>>(),
        vec![source_a.as_str(), source_z.as_str()]
    );
    assert_eq!(
        document
            .wallpapers
            .iter()
            .map(|wallpaper| wallpaper.path.as_str())
            .collect::<Vec<_>>(),
        vec![wallpaper_a.as_str(), wallpaper_z.as_str()]
    );
    assert_eq!(
        document
            .tags
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Zebra"]
    );
    assert_eq!(
        document
            .collections
            .iter()
            .map(|collection| collection.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Archive", "Travel"]
    );

    let first = &document.wallpapers[0];
    assert_eq!(first.display_title, None);
    let second = &document.wallpapers[1];
    assert_eq!(second.display_title.as_deref(), Some("Zed title"));
    assert_eq!(second.rating, 1);
    assert!(second.hidden);
    assert_eq!(second.width, 2560);
    assert_eq!(second.height, 1440);
    assert_eq!(second.file_size, 42);
    assert_eq!(second.tags, vec!["Alpha", "Zebra"]);
    assert_eq!(second.collections, vec!["Archive", "Travel"]);
    assert_eq!(document.settings.rotation_secs, Some(900));
    assert_eq!(document.settings.display_mode.as_deref(), Some("span"));
    assert_eq!(document.settings.focus_mode_enabled, Some(true));
    assert_eq!(document.settings.paused, Some(false));
    assert_eq!(document.settings.theme.as_deref(), Some("dark"));
    assert_eq!(
        document.settings.workspace_mode.as_deref(),
        Some("workbench")
    );

    let bytes_a = serialize_bounded(&document).expect("snapshot should serialize");
    let bytes_b = serialize_bounded(&document).expect("snapshot should serialize again");
    assert_eq!(bytes_a, bytes_b);
    let text = String::from_utf8(bytes_a).expect("backup JSON should be UTF-8");
    assert!(text.starts_with("{\n  \"app\": \"PureWall\""));
    assert!(text.ends_with('\n'));
    for forbidden in [
        "playCount",
        "lastPlayed",
        "playEvents",
        "fileAvailable",
        "maintenance_secret",
        "do-not-export",
        "registry",
        "autostart",
    ] {
        assert!(
            !text.contains(forbidden),
            "backup must omit forbidden field/value {forbidden}"
        );
    }

    drop(database);
    remove_fixture(&root);
}

#[test]
fn bounded_serialization_and_reading_reject_oversized_data() {
    let root = unique_temp_path("bounded-io");
    remove_fixture(&root);
    fs::create_dir_all(&root).expect("test root should exist");
    let oversized = vec![b'x'; MAX_BACKUP_BYTES as usize + 1];
    let oversized_path = root.join("oversized.json");
    fs::write(&oversized_path, &oversized).expect("oversized fixture should persist");
    assert_eq!(
        read_bounded(&oversized_path)
            .expect_err("oversized file should be rejected")
            .code(),
        "BACKUP_TOO_LARGE"
    );

    let mut document = parse(&valid_backup())
        .expect("base backup should parse")
        .document;
    document.app = "x".repeat(MAX_BACKUP_BYTES as usize + 1);
    assert_eq!(
        serialize_bounded(&document)
            .expect_err("oversized output should be rejected")
            .code(),
        "BACKUP_TOO_LARGE"
    );

    remove_fixture(&root);
}

#[test]
fn atomic_writer_rejects_before_mutation_and_replaces_existing_destination() {
    let root = unique_temp_path("atomic-write");
    remove_fixture(&root);
    fs::create_dir_all(&root).expect("test root should exist");
    let destination = root.join("backup.json");
    fs::write(&destination, b"old-complete-json").expect("old destination should exist");
    let oversized = vec![b'x'; MAX_BACKUP_BYTES as usize + 1];

    let error =
        write_atomically(&destination, &oversized).expect_err("oversized write should fail");
    assert_eq!(error.code(), "BACKUP_TOO_LARGE");
    assert_eq!(
        fs::read(&destination).expect("old destination should remain readable"),
        b"old-complete-json"
    );

    let replacement = b"{\n  \"complete\": true\n}\n";
    write_atomically(&destination, replacement).expect("replacement should install atomically");
    assert_eq!(
        fs::read(&destination).expect("new destination should remain readable"),
        replacement
    );
    assert_eq!(
        fs::read_dir(&root)
            .expect("test root should remain readable")
            .count(),
        1,
        "successful replacement must not leave an operation temp file"
    );

    remove_fixture(&root);
}

#[test]
fn export_destination_rejects_registered_wallpapers_non_json_and_app_data() {
    let root = unique_temp_path("export-destination");
    remove_fixture(&root);
    fs::create_dir_all(&root).expect("test root should exist");
    let registered_wallpaper = root.join("registered.json");
    fs::write(&registered_wallpaper, b"original-wallpaper")
        .expect("registered wallpaper fixture should exist");
    let protected_wallpapers = vec![registered_wallpaper.to_string_lossy().to_string()];
    let app_data_root = root.join("app-data");
    fs::create_dir_all(&app_data_root).expect("app-data fixture should exist");

    let registered_error = validate_export_destination(
        &registered_wallpaper,
        &protected_wallpapers,
        Some(&app_data_root),
    )
    .expect_err("a registered wallpaper must never be an export destination");
    assert_eq!(registered_error.code(), "BACKUP_UNSAFE_DESTINATION");
    assert_eq!(
        fs::read(&registered_wallpaper).expect("registered wallpaper should remain readable"),
        b"original-wallpaper"
    );

    let extension_error =
        validate_export_destination(&root.join("backup.jpg"), &[], Some(&app_data_root))
            .expect_err("a native caller must not bypass the JSON destination contract");
    assert_eq!(extension_error.code(), "BACKUP_UNSAFE_DESTINATION");

    let app_data_error = validate_export_destination(
        &app_data_root.join("purewall-backup-v1.json"),
        &[],
        Some(&app_data_root),
    )
    .expect_err("application data must not be replaced by backup export");
    assert_eq!(app_data_error.code(), "BACKUP_UNSAFE_DESTINATION");

    validate_export_destination(
        &root.join("purewall-backup-v1.json"),
        &protected_wallpapers,
        Some(&app_data_root),
    )
    .expect("a JSON destination outside protected paths should be accepted");

    remove_fixture(&root);
}

#[test]
fn preview_digest_rejects_replaced_backup_bytes() {
    let previewed = br#"{"app":"PureWall","schemaVersion":1}"#;
    let replacement = br#"{"app":"PureWall","schemaVersion":1,"wallpapers":[{"path":"changed"}]}"#;
    let digest = backup_content_digest(previewed);

    verify_preview_digest(previewed, &digest).expect("unchanged preview bytes should be accepted");
    let error = verify_preview_digest(replacement, &digest)
        .expect_err("replaced backup bytes must require a fresh preview");
    assert_eq!(error.code(), "BACKUP_PREVIEW_CHANGED");
}

#[test]
fn atomic_writer_removes_only_its_temp_file_when_installation_fails() {
    let root = unique_temp_path("atomic-cleanup");
    remove_fixture(&root);
    fs::create_dir_all(&root).expect("test root should exist");
    let destination = root.join("destination-is-a-directory");
    fs::create_dir_all(&destination).expect("failure destination should be a directory");
    let unrelated = root.join("unrelated.tmp");
    fs::write(&unrelated, b"keep-me").expect("unrelated fixture should exist");

    let error = write_atomically(&destination, b"complete payload")
        .expect_err("installing over a directory should fail");
    assert_eq!(error.code(), "BACKUP_WRITE_FAILED");
    assert_eq!(
        fs::read(&unrelated).expect("unrelated file must remain"),
        b"keep-me"
    );
    assert_eq!(
        fs::read_dir(&root)
            .expect("test root should remain readable")
            .count(),
        2,
        "failed installation must remove only its operation-owned temp file"
    );

    remove_fixture(&root);
}

fn backup_wallpaper(
    path: &str,
    source: &str,
    hash: &str,
    display_title: Option<&str>,
    user_state: (i32, bool),
    tags: Vec<&str>,
    collections: Vec<&str>,
) -> Value {
    json!({
        "path": path,
        "source": source,
        "hash": hash,
        "displayTitle": display_title,
        "rating": user_state.0,
        "hidden": user_state.1,
        "width": 800,
        "height": 600,
        "fileSize": 1234,
        "tags": tags,
        "collections": collections
    })
}

#[test]
fn previews_counts_conflicts_and_missing_paths_without_mutating_sqlite() {
    let root = unique_temp_path("preview");
    remove_fixture(&root);
    fs::create_dir_all(root.join("source")).expect("source should exist");
    let existing_file = root.join("existing.jpg");
    fs::write(&existing_file, b"existing").expect("existing wallpaper should exist");
    let existing_path = existing_file
        .canonicalize()
        .expect("existing wallpaper should canonicalize")
        .to_string_lossy()
        .to_string();
    let missing_path = root.join("missing.jpg").to_string_lossy().to_string();
    let source_path = root
        .join("source")
        .canonicalize()
        .expect("source should canonicalize")
        .to_string_lossy()
        .to_string();
    let database = Database::new(root.join("purewall.db")).expect("test database should open");
    database
        .upsert_wallpaper(&existing_path, "local", "mounted", 10, 20, 30)
        .expect("existing wallpaper metadata should persist");

    let mut value = valid_backup();
    value["sources"] = json!([{
        "path": source_path,
        "source": "mounted"
    }]);
    value["tags"] = json!([{ "name": "Nature", "color": "#008800" }]);
    value["collections"] = json!([{ "name": "Desk", "color": "#112233" }]);
    value["wallpapers"] = json!([
        backup_wallpaper(
            &existing_path,
            "mounted",
            "backup-existing",
            Some("Imported title"),
            (1, true),
            vec!["Nature"],
            vec!["Desk"],
        ),
        backup_wallpaper(
            &missing_path,
            "imported-file",
            "backup-missing",
            None,
            (0, false),
            vec![],
            vec![],
        )
    ]);
    value["settings"] = json!({
        "rotationSecs": 900,
        "theme": "dark",
        "unknownSetting": "ignored"
    });
    let backup = parse(&value).expect("preview backup should normalize");

    let preview = database
        .preview_backup_import(&backup, "sha256:preview-fixture".to_string())
        .expect("preview should succeed");
    assert_eq!(preview.content_digest, "sha256:preview-fixture");
    assert_eq!(preview.source_count, 1);
    assert_eq!(preview.wallpaper_count, 2);
    assert_eq!(preview.tag_count, 1);
    assert_eq!(preview.collection_count, 1);
    assert_eq!(preview.setting_count, 2);
    assert_eq!(preview.new_wallpapers, 1);
    assert_eq!(preview.overwritten_wallpapers, 1);
    assert_eq!(preview.missing_paths, 1);
    assert_eq!(preview.warnings.len(), 1);
    assert!(preview.warnings[0].contains('1'));

    assert!(
        database
            .get_watched_folders()
            .expect("sources should remain readable")
            .is_empty(),
        "preview must not add backup sources"
    );
    assert!(
        database
            .get_wallpaper_by_path(&missing_path)
            .expect("missing metadata query should succeed")
            .is_none(),
        "preview must not add backup wallpaper metadata"
    );
    assert_eq!(
        database
            .get_wallpaper_by_path(&existing_path)
            .expect("existing metadata query should succeed")
            .expect("existing wallpaper should remain")
            .rating,
        0,
        "preview must not overwrite existing metadata"
    );
    assert_eq!(
        database
            .get_setting("rotation_secs")
            .expect("settings should remain readable"),
        None,
        "preview must not persist backup settings"
    );

    drop(database);
    remove_fixture(&root);
}

#[test]
fn merge_overrides_user_metadata_preserves_local_fields_and_is_idempotent() {
    let root = unique_temp_path("merge");
    remove_fixture(&root);
    for folder in ["source-existing", "source-new", "source-local-only"] {
        fs::create_dir_all(root.join(folder)).expect("source fixture should exist");
    }
    let existing_file = root.join("existing.jpg");
    let local_only_file = root.join("local-only.jpg");
    fs::write(&existing_file, b"existing").expect("existing wallpaper should exist");
    fs::write(&local_only_file, b"local-only").expect("local-only wallpaper should exist");
    let existing_path = existing_file
        .canonicalize()
        .expect("existing wallpaper should canonicalize")
        .to_string_lossy()
        .to_string();
    let local_only_path = local_only_file
        .canonicalize()
        .expect("local-only wallpaper should canonicalize")
        .to_string_lossy()
        .to_string();
    let missing_path = root.join("offline.jpg").to_string_lossy().to_string();
    let source_existing = root
        .join("source-existing")
        .canonicalize()
        .expect("existing source should canonicalize")
        .to_string_lossy()
        .to_string();
    let source_new = root
        .join("source-new")
        .canonicalize()
        .expect("new source should canonicalize")
        .to_string_lossy()
        .to_string();
    let source_local_only = root
        .join("source-local-only")
        .canonicalize()
        .expect("local-only source should canonicalize")
        .to_string_lossy()
        .to_string();
    let mut database = Database::new(root.join("purewall.db")).expect("test database should open");

    database
        .upsert_watched_folder(&source_existing, "mounted")
        .expect("existing source should persist");
    database
        .upsert_watched_folder(&source_local_only, "mounted")
        .expect("local-only source should persist");
    database
        .upsert_wallpaper(&existing_path, "local-hash", "mounted", 111, 222, 333)
        .expect("existing wallpaper should persist");
    database
        .upsert_wallpaper(&local_only_path, "local-only-hash", "dropped", 10, 20, 30)
        .expect("local-only wallpaper should persist");
    database
        .set_rating(&existing_path, -1)
        .expect("local rating should persist");
    database
        .set_wallpaper_display_title(&existing_path, "Local title")
        .expect("local title should persist");
    let old_tag = database
        .create_tag("OldTag", "#999999")
        .expect("old tag should persist");
    database
        .create_tag("Shared", "#local-tag")
        .expect("shared tag should persist");
    database
        .assign_tag(&existing_path, old_tag.id)
        .expect("old tag relation should persist");
    let old_collection = database
        .create_collection("OldCollection", "#888888")
        .expect("old collection should persist");
    database
        .create_collection("SharedCollection", "#local-collection")
        .expect("shared collection should persist");
    database
        .assign_collection(&existing_path, old_collection.id)
        .expect("old collection relation should persist");
    database
        .set_setting("rotation_secs", "600")
        .expect("local rotation should persist");
    database
        .set_setting("display_mode", "all")
        .expect("local display mode should persist");
    database
        .set_setting("local_only_setting", "keep")
        .expect("local-only setting should persist");

    let mut value = valid_backup();
    value["sources"] = json!([
        { "path": source_new, "source": "imported-folder" },
        { "path": source_existing, "source": "mounted" }
    ]);
    value["tags"] = json!([
        { "name": "NewTag", "color": "#new-tag" },
        { "name": "Shared", "color": "#backup-tag" }
    ]);
    value["collections"] = json!([
        { "name": "NewCollection", "color": "#new-collection" },
        { "name": "SharedCollection", "color": "#backup-collection" }
    ]);
    value["wallpapers"] = json!([
        backup_wallpaper(
            &existing_path,
            "imported-file",
            "backup-hash",
            None,
            (1, true),
            vec!["NewTag", "Shared"],
            vec!["NewCollection", "SharedCollection"],
        ),
        backup_wallpaper(
            &missing_path,
            "imported-file",
            "missing-hash",
            Some("Offline"),
            (0, false),
            vec!["NewTag"],
            vec!["NewCollection"],
        )
    ]);
    value["settings"] = json!({
        "rotationSecs": 900,
        "displayMode": "span",
        "focusModeEnabled": true,
        "paused": false,
        "theme": "dark",
        "workspaceMode": "quiet"
    });
    let backup = parse(&value).expect("merge backup should normalize");

    let first = database
        .merge_backup(&backup)
        .expect("first merge should succeed");
    assert_eq!(first.added_wallpapers, 1);
    assert_eq!(first.updated_wallpapers, 1);
    assert_eq!(first.added_sources, 1);
    assert_eq!(first.created_tags, 1);
    assert_eq!(first.created_collections, 1);
    assert_eq!(first.updated_settings, 4);
    assert_eq!(first.client_settings.theme.as_deref(), Some("dark"));
    assert_eq!(
        first.client_settings.workspace_mode.as_deref(),
        Some("quiet")
    );

    let existing = database
        .get_wallpaper_by_path(&existing_path)
        .expect("existing metadata query should succeed")
        .expect("existing wallpaper should remain");
    assert_eq!(existing.rating, 1);
    assert!(existing.blacklisted);
    assert_eq!(existing.display_title, "");
    assert_eq!(existing.hash, "local-hash");
    assert_eq!(existing.source, "mounted");
    assert_eq!(
        (existing.width, existing.height, existing.file_size),
        (111, 222, 333)
    );
    let mut existing_tags = existing
        .tags
        .iter()
        .map(|tag| tag.name.as_str())
        .collect::<Vec<_>>();
    existing_tags.sort_unstable();
    assert_eq!(existing_tags, vec!["NewTag", "Shared"]);

    let local_only = database
        .get_wallpaper_by_path(&local_only_path)
        .expect("local-only query should succeed")
        .expect("local-only wallpaper should remain");
    assert_eq!(local_only.hash, "local-only-hash");
    assert_eq!(local_only.rating, 0);

    let offline = database
        .get_wallpaper_by_path(&missing_path)
        .expect("offline metadata query should succeed")
        .expect("offline metadata should be inserted");
    assert_eq!(offline.hash, "missing-hash");
    assert!(
        database
            .get_all_wallpapers()
            .expect("visible wallpapers should remain readable")
            .iter()
            .all(|wallpaper| wallpaper.path != missing_path),
        "a missing imported path must remain unavailable"
    );

    assert_eq!(
        database
            .get_setting("local_only_setting")
            .expect("local-only setting should remain readable")
            .as_deref(),
        Some("keep")
    );
    assert_eq!(
        database
            .get_setting("rotation_secs")
            .expect("rotation should remain readable")
            .as_deref(),
        Some("900")
    );

    let snapshot = database
        .export_backup_snapshot(
            BackupClientSettings::default(),
            "2026-07-29T13:00:00Z".to_string(),
        )
        .expect("merged state should export");
    assert_eq!(snapshot.sources.len(), 3, "local-only source must remain");
    assert_eq!(
        snapshot
            .tags
            .iter()
            .find(|tag| tag.name == "Shared")
            .expect("shared tag should be reused")
            .color,
        "#local-tag"
    );
    assert_eq!(
        snapshot
            .collections
            .iter()
            .find(|collection| collection.name == "SharedCollection")
            .expect("shared collection should be reused")
            .color,
        "#local-collection"
    );
    let merged_existing = snapshot
        .wallpapers
        .iter()
        .find(|wallpaper| wallpaper.path == existing_path)
        .expect("merged wallpaper should export");
    assert_eq!(
        merged_existing.collections,
        vec!["NewCollection", "SharedCollection"]
    );

    let second = database
        .merge_backup(&backup)
        .expect("second merge should be idempotent");
    assert_eq!(second.added_wallpapers, 0);
    assert_eq!(second.updated_wallpapers, 0);
    assert_eq!(second.added_sources, 0);
    assert_eq!(second.created_tags, 0);
    assert_eq!(second.created_collections, 0);
    assert_eq!(second.updated_settings, 0);

    drop(database);
    remove_fixture(&root);
}

#[test]
fn merge_rolls_back_sources_definitions_wallpapers_and_settings_on_relation_error() {
    let root = unique_temp_path("merge-rollback");
    remove_fixture(&root);
    fs::create_dir_all(root.join("new-source")).expect("new source should exist");
    let source = root
        .join("new-source")
        .canonicalize()
        .expect("new source should canonicalize")
        .to_string_lossy()
        .to_string();
    let wallpaper = root.join("new-wall.jpg").to_string_lossy().to_string();
    let mut database = Database::new(root.join("purewall.db")).expect("test database should open");
    database
        .set_setting("rotation_secs", "600")
        .expect("local setting should persist");

    let mut value = valid_backup();
    value["sources"] = json!([{ "path": source, "source": "mounted" }]);
    value["tags"] = json!([{ "name": "DefinedTag", "color": "#111111" }]);
    value["collections"] = json!([{ "name": "DefinedCollection", "color": "#222222" }]);
    value["wallpapers"] = json!([backup_wallpaper(
        &wallpaper,
        "mounted",
        "new-hash",
        Some("New"),
        (1, false),
        vec!["DefinedTag"],
        vec!["DefinedCollection"],
    )]);
    value["settings"] = json!({ "rotationSecs": 900 });
    let mut backup = parse(&value).expect("rollback backup should initially normalize");
    backup.document.wallpapers[0]
        .tags
        .push("UndefinedAfterNormalization".to_string());

    database
        .merge_backup(&backup)
        .expect_err("undefined relation should roll back the transaction");

    assert!(database
        .get_watched_folders()
        .expect("sources should remain readable")
        .is_empty());
    assert!(database
        .get_tags()
        .expect("tags should remain readable")
        .is_empty());
    assert!(database
        .get_collections()
        .expect("collections should remain readable")
        .is_empty());
    assert!(database
        .get_wallpaper_by_path(&wallpaper)
        .expect("wallpaper metadata should remain readable")
        .is_none());
    assert_eq!(
        database
            .get_setting("rotation_secs")
            .expect("setting should remain readable")
            .as_deref(),
        Some("600")
    );

    drop(database);
    remove_fixture(&root);
}

#[test]
fn maps_every_backup_error_to_its_stable_command_code() {
    let oversized = vec![b'x'; MAX_BACKUP_BYTES as usize + 1];
    let too_large = parse_and_normalize(&oversized).expect_err("oversized input should fail");

    let mut wrong_app = valid_backup();
    wrong_app["app"] = json!("OtherWall");
    let invalid_app = parse(&wrong_app).expect_err("wrong app should fail");

    let mut future_schema = valid_backup();
    future_schema["schemaVersion"] = json!(2);
    let unsupported_schema = parse(&future_schema).expect_err("future schema should fail");

    let mut invalid_data_value = valid_backup();
    invalid_data_value["sources"] = json!([{ "path": "relative/source", "source": "mounted" }]);
    let invalid_data = parse(&invalid_data_value).expect_err("relative path should fail");

    let missing = unique_temp_path("missing-backup").join("not-found.json");
    let write_failed = read_bounded(&missing).expect_err("missing backup should fail");

    for (error, expected) in [
        (too_large, "BACKUP_TOO_LARGE"),
        (invalid_app, "BACKUP_INVALID_APP"),
        (unsupported_schema, "BACKUP_UNSUPPORTED_SCHEMA"),
        (invalid_data, "BACKUP_INVALID_DATA"),
        (write_failed, "BACKUP_WRITE_FAILED"),
    ] {
        let command_error = crate::backup_command_error(error);
        assert_eq!(command_error.code, expected);
        assert!(command_error.message.contains(expected));
    }
}

#[test]
fn post_commit_source_reconciliation_returns_warnings_without_changing_database() {
    let root = unique_temp_path("post-commit-reconcile");
    remove_fixture(&root);
    fs::create_dir_all(root.join("reachable")).expect("reachable source should exist");
    let reachable = root
        .join("reachable")
        .canonicalize()
        .expect("reachable source should canonicalize")
        .to_string_lossy()
        .to_string();
    let offline = root.join("offline").to_string_lossy().to_string();
    let mut value = valid_backup();
    value["sources"] = json!([
        { "path": reachable, "source": "mounted" },
        { "path": offline, "source": "imported-folder" }
    ]);
    let backup = parse(&value).expect("source backup should normalize");
    let mut database = Database::new(root.join("purewall.db")).expect("test database should open");
    database
        .merge_backup(&backup)
        .expect("source metadata should commit before reconciliation");
    let before = database
        .export_backup_snapshot(
            BackupClientSettings::default(),
            "2026-07-29T14:00:00Z".to_string(),
        )
        .expect("committed state should export");

    let mut attempted = Vec::new();
    let reconciliation = reconcile_persisted_sources(&before.sources, |path, source| {
        attempted.push((path.to_string(), source.to_string()));
        Err("injected watcher start failure".to_string())
    });

    assert_eq!(attempted, vec![(reachable.clone(), "mounted".to_string())]);
    assert_eq!(reconciliation.warnings.len(), 2);
    assert_eq!(reconciliation.failures.len(), 2);
    assert!(reconciliation
        .warnings
        .iter()
        .any(|warning| warning.contains("offline")));
    assert!(reconciliation
        .warnings
        .iter()
        .any(|warning| warning.contains("injected watcher start failure")));

    let after = database
        .export_backup_snapshot(
            BackupClientSettings::default(),
            "2026-07-29T14:00:00Z".to_string(),
        )
        .expect("committed state should remain exportable");
    assert_eq!(
        after, before,
        "post-commit reconciliation must not rewrite the merged database"
    );

    drop(database);
    remove_fixture(&root);
}

#[cfg(windows)]
fn absolute_missing_path(name: &str) -> String {
    format!(r"C:\PureWall-Offline\{name}")
}

#[cfg(not(windows))]
fn absolute_missing_path(name: &str) -> String {
    format!("/purewall-offline/{name}")
}
