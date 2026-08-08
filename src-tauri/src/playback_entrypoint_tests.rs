use crate::playback_action::{PlaybackAction, PlaybackActionOutcome};
use crate::{
    cli_action_argument, cli_action_from_args, legacy_pause_state, legacy_wallpaper_path,
    playback_completion_events, playback_error_code, should_deliver_playback_completion,
};
use tauri::EventTarget;

#[test]
fn cli_actions_use_the_shared_playback_vocabulary() {
    let args = vec![
        "purewall.exe".to_string(),
        "--action".to_string(),
        "like".to_string(),
    ];
    assert_eq!(cli_action_from_args(&args), Some(PlaybackAction::Like));
}

#[test]
fn unknown_cli_actions_are_rejected_before_execution() {
    let args = vec![
        "purewall.exe".to_string(),
        "--action".to_string(),
        "shuffle".to_string(),
    ];
    assert_eq!(cli_action_from_args(&args), None);
}
#[test]
fn playback_error_codes_keep_missing_candidates_distinct_from_platform_failures() {
    assert_eq!(
        playback_error_code("No existing wallpaper files found"),
        "playback_unavailable"
    );
    assert_eq!(
        playback_error_code("COM error 0x80004005 while setting wallpaper"),
        "operation_failed"
    );
}

#[test]
fn legacy_facades_extract_only_the_outcome_their_action_promises() {
    for (action, rating) in [
        (PlaybackAction::Next, None),
        (PlaybackAction::Like, Some(1)),
        (PlaybackAction::Dislike, Some(-1)),
    ] {
        let outcome = PlaybackActionOutcome {
            action,
            current_wallpaper_path: Some("D:/walls/current.jpg".to_string()),
            rating,
            paused: None,
        };
        assert_eq!(
            legacy_wallpaper_path(outcome).expect("wallpaper facade result"),
            "D:/walls/current.jpg"
        );
    }

    let paused = PlaybackActionOutcome {
        action: PlaybackAction::TogglePause,
        current_wallpaper_path: None,
        rating: None,
        paused: Some(true),
    };
    assert!(legacy_wallpaper_path(paused.clone()).is_err());
    assert!(legacy_pause_state(paused).expect("pause facade result"));
}

#[test]
fn each_action_maps_to_one_success_event_or_one_failure_report() {
    assert_eq!(
        playback_completion_events(PlaybackAction::Next, true),
        ["auto-rotated"]
    );
    assert_eq!(
        playback_completion_events(PlaybackAction::Like, true),
        ["wallpaper-rating-changed"]
    );
    assert_eq!(
        playback_completion_events(PlaybackAction::Dislike, true),
        ["wallpaper-rating-changed"]
    );
    assert_eq!(
        playback_completion_events(PlaybackAction::TogglePause, true),
        ["pause-changed"]
    );
    for action in [
        PlaybackAction::Next,
        PlaybackAction::Like,
        PlaybackAction::Dislike,
        PlaybackAction::TogglePause,
    ] {
        assert_eq!(
            playback_completion_events(action, false),
            ["operation-failed"]
        );
    }
}

#[test]
fn cli_pause_keeps_the_legacy_token_for_failure_logging() {
    let args = vec![
        "purewall.exe".to_string(),
        "--action".to_string(),
        "pause".to_string(),
    ];
    let action = cli_action_from_args(&args).expect("pause is a supported CLI action");
    assert_eq!(action, PlaybackAction::TogglePause);
    assert_eq!(cli_action_argument(action), "pause");
}

#[test]
fn broadcast_completion_delivery_allows_every_event_target() {
    for target in [
        EventTarget::Any,
        EventTarget::AnyLabel {
            label: "main".into(),
        },
        EventTarget::App,
        EventTarget::Window {
            label: "main".into(),
        },
        EventTarget::Webview {
            label: "main".into(),
        },
        EventTarget::WebviewWindow {
            label: "main".into(),
        },
    ] {
        assert!(should_deliver_playback_completion(&target, None));
    }
}

#[test]
fn caller_scoped_completion_excludes_every_main_label_target_but_keeps_widget_and_app() {
    for target in [
        EventTarget::AnyLabel {
            label: "main".into(),
        },
        EventTarget::Window {
            label: "main".into(),
        },
        EventTarget::Webview {
            label: "main".into(),
        },
        EventTarget::WebviewWindow {
            label: "main".into(),
        },
    ] {
        assert!(!should_deliver_playback_completion(&target, Some("main")));
    }

    for target in [
        EventTarget::Any,
        EventTarget::App,
        EventTarget::AnyLabel {
            label: "widget".into(),
        },
        EventTarget::Window {
            label: "widget".into(),
        },
        EventTarget::Webview {
            label: "widget".into(),
        },
        EventTarget::WebviewWindow {
            label: "widget".into(),
        },
    ] {
        assert!(should_deliver_playback_completion(&target, Some("main")));
    }
}

#[test]
fn independent_display_partial_failure_commits_each_success_before_returning_error() {
    use crate::{
        apply_independent_display_assignments_with, db::Database,
        playback_state_tests::TempPlaybackFiles,
    };
    use chrono::Datelike;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_nanos();
    let prefix = format!("purewall-partial-display-{}-{unique}", std::process::id());
    let db_path = std::env::temp_dir().join(format!("{prefix}.db"));
    let first_path = std::env::temp_dir().join(format!("{prefix}-a.jpg"));
    let second_path = std::env::temp_dir().join(format!("{prefix}-b.jpg"));
    let _files = TempPlaybackFiles::new(
        db_path.clone(),
        vec![first_path.clone(), second_path.clone()],
    );
    std::fs::write(&first_path, b"first").expect("first wallpaper should be writable");
    std::fs::write(&second_path, b"second").expect("second wallpaper should be writable");
    let first = first_path.to_string_lossy().to_string();
    let second = second_path.to_string_lossy().to_string();
    let db = Database::new(&db_path).expect("database should initialize");
    db.upsert_wallpaper(&first, "partial-a", "test", 100, 100, 5)
        .expect("first wallpaper should register");
    db.upsert_wallpaper(&second, "partial-b", "test", 100, 100, 6)
        .expect("second wallpaper should register");
    let assignments = vec![
        ("DISPLAY-A".to_string(), first.clone()),
        ("DISPLAY-B".to_string(), second.clone()),
    ];

    let mut attempts = Vec::new();
    let error = apply_independent_display_assignments_with(&db, &assignments, |display, path| {
        attempts.push((display.to_string(), path.to_string()));
        if display == "DISPLAY-B" {
            Err("DISPLAY-B platform apply failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect_err("the second platform apply should fail");

    assert_eq!(error, "DISPLAY-B platform apply failed");
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        db.get_setting("current_wallpaper")
            .expect("current setting should load")
            .as_deref(),
        Some(first.as_str())
    );
    let stats = db
        .get_yearly_stats(chrono::Utc::now().year())
        .expect("history should load");
    assert_eq!(stats.total_plays, 1);
    assert_eq!(stats.unique_wallpapers, 1);
    assert_eq!(stats.top_wallpapers.len(), 1);
    assert_eq!(stats.top_wallpapers[0].path, first);
    assert_eq!(stats.top_wallpapers[0].plays, 1);
}

#[test]
fn temp_playback_files_remove_precreated_artifacts_during_expected_unwind() {
    use crate::playback_state_tests::TempPlaybackFiles;
    use std::time::{SystemTime, UNIX_EPOCH};
    const CLEANUP_PANIC_SENTINEL: &str = "purewall-temp-playback-cleanup-sentinel";

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_nanos();
    let prefix = format!("purewall-panic-cleanup-{}-{unique}", std::process::id());
    let db_path = std::env::temp_dir().join(format!("{prefix}.db"));
    let first_path = std::env::temp_dir().join(format!("{prefix}-a.jpg"));
    let second_path = std::env::temp_dir().join(format!("{prefix}-b.jpg"));
    let expected_removed_paths = [
        first_path.clone(),
        second_path.clone(),
        db_path.clone(),
        db_path.with_extension("db-shm"),
        db_path.with_extension("db-wal"),
    ];
    let files = TempPlaybackFiles::new(db_path, vec![first_path, second_path]);
    for path in &expected_removed_paths {
        std::fs::write(path, b"temporary test artifact")
            .expect("temporary artifact should be writable");
    }
    for path in &expected_removed_paths {
        assert!(
            path.exists(),
            "temporary artifact setup should exist before unwind: {}",
            path.display()
        );
    }

    let unwind = std::panic::catch_unwind(move || {
        let _files = files;
        std::panic::panic_any(CLEANUP_PANIC_SENTINEL);
    });

    let panic_payload = unwind.expect_err("cleanup exercise should unwind");
    let panic_message = if let Some(message) = panic_payload.downcast_ref::<&str>() {
        *message
    } else if let Some(message) = panic_payload.downcast_ref::<String>() {
        message.as_str()
    } else {
        panic!("cleanup exercise returned an unexpected panic payload type");
    };
    assert_eq!(
        panic_message, CLEANUP_PANIC_SENTINEL,
        "cleanup must be verified only for the deliberate sentinel panic"
    );

    for path in expected_removed_paths {
        assert!(
            !path.exists(),
            "temporary artifact should be removed during unwind: {}",
            path.display()
        );
    }
}

#[test]
fn committed_next_remains_successful_when_completion_emit_fails() {
    use crate::finish_playback_action_with_best_effort_completion;
    use std::cell::{Cell, RefCell};

    let side_effect_committed = Cell::new(false);
    let observed_emit_failures = RefCell::new(Vec::new());
    let outcome = finish_playback_action_with_best_effort_completion(
        || {
            side_effect_committed.set(true);
            Ok(PlaybackActionOutcome {
                action: PlaybackAction::Next,
                current_wallpaper_path: Some("D:/walls/committed.jpg".to_string()),
                rating: None,
                paused: None,
            })
        },
        |_outcome| Err("webview completion delivery failed".to_string()),
        |error| observed_emit_failures.borrow_mut().push(error.to_string()),
    )
    .expect("post-commit emit failure must not fail Next");

    assert!(side_effect_committed.get());
    assert_eq!(outcome.action, PlaybackAction::Next);
    assert_eq!(
        outcome.current_wallpaper_path.as_deref(),
        Some("D:/walls/committed.jpg")
    );
    assert_eq!(
        observed_emit_failures.borrow().as_slice(),
        ["webview completion delivery failed"]
    );
}
