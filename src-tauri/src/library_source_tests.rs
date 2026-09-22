use crate::library_sources::{status_for, LibrarySourceStatus, SourceOperationGuard};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[test]
fn source_status_prioritizes_scanning_then_offline_then_error() {
    let path = std::env::temp_dir()
        .canonicalize()
        .expect("temp root should canonicalize")
        .to_string_lossy()
        .to_string();
    let mut scanning = HashSet::new();
    scanning.insert(path.clone());
    assert_eq!(
        status_for(&path, &scanning, Some("old error")),
        LibrarySourceStatus::Scanning,
    );

    scanning.clear();
    assert_eq!(
        status_for(&path, &scanning, Some("old error")),
        LibrarySourceStatus::Error,
    );
    assert_eq!(
        status_for(
            r"Z:\purewall-source-that-does-not-exist",
            &scanning,
            Some("old error"),
        ),
        LibrarySourceStatus::Offline,
    );
}

#[test]
fn source_operation_guard_releases_after_scope_and_unwind() {
    let operations = Mutex::new(HashSet::new());
    {
        let _guard = SourceOperationGuard::begin(&operations, "D:\\Walls")
            .expect("first source operation should begin");
        assert!(operations.lock().unwrap().contains("D:\\Walls"));
        assert!(SourceOperationGuard::begin(&operations, "D:\\Walls").is_err());
    }
    assert!(operations.lock().unwrap().is_empty());

    let guard = SourceOperationGuard::begin(&operations, "D:\\Walls")
        .expect("unwind source operation should begin");
    assert!(operations.lock().unwrap().contains("D:\\Walls"));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = guard;
        panic!("source-operation-sentinel");
    }));
    let payload = unwind.expect_err("sentinel panic should be captured");
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
    assert_eq!(message, Some("source-operation-sentinel"));
    assert!(operations.lock().unwrap().is_empty());
}

#[test]
fn source_error_mapping_uses_stable_uppercase_codes() {
    assert_eq!(
        crate::source_database_error("relocation collision: occupied").code,
        "SOURCE_RELOCATE_COLLISION",
    );
    assert_eq!(
        crate::source_database_error("source path conflict: overlap").code,
        "SOURCE_PATH_CONFLICT",
    );
    assert_eq!(
        crate::source_path_error(
            r"Z:\purewall-source-that-does-not-exist",
            "Folder does not exist",
            "SOURCE_OFFLINE",
        )
        .code,
        "SOURCE_OFFLINE",
    );
}

#[test]
fn stale_watcher_callback_cannot_match_a_changed_registration() {
    let registrations = vec![crate::db::WatchedFolderEntry {
        path: "D:\\Walls".to_string(),
        source: "mounted".to_string(),
        last_scan_at: None,
        last_error: None,
    }];
    assert!(crate::watched_folder_registration_is_current(
        &registrations,
        "d:/walls",
        "mounted",
    ));
    assert!(!crate::watched_folder_registration_is_current(
        &registrations,
        "D:\\Walls",
        "imported-folder",
    ));
    assert!(!crate::watched_folder_registration_is_current(
        &registrations,
        "D:\\Removed",
        "mounted",
    ));
}

#[test]
fn taking_a_watcher_removes_only_the_exact_canonical_root() {
    let mut watchers = HashMap::from([
        ("D:\\Walls".to_string(), ("mounted".to_string(), 1)),
        (
            "D:\\Walls-Other".to_string(),
            ("imported-folder".to_string(), 2),
        ),
    ]);
    let removed = crate::take_runtime_folder_watcher(&mut watchers, "D:\\Walls");
    assert_eq!(removed, Some(("mounted".to_string(), 1)));
    assert_eq!(watchers.len(), 1);
    assert!(watchers.contains_key("D:\\Walls-Other"));
}
