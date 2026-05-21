use prost::Message as _;

use super::*;

#[test]
fn parse_bytes_accepts_expected_suffixes() {
    assert_eq!(parse_bytes("1").unwrap(), 1);
    assert_eq!(parse_bytes("2KiB").unwrap(), 2 * 1024);
    assert_eq!(parse_bytes("3MiB").unwrap(), 3 * 1024 * 1024);
    assert_eq!(parse_bytes("4GiB").unwrap(), 4 * 1024 * 1024 * 1024);
    assert!(parse_bytes("1TiB").is_err());
}

#[test]
fn generated_tasks_round_trip_into_passive_request() {
    let tasks = build_conversation_tasks(0, 3, 1, 2, 128);
    assert_eq!(tasks.len(), 3);

    let decoded_tasks = tasks
        .iter()
        .map(|task| {
            let encoded = task.encode_to_vec();
            api::Task::decode(encoded.as_slice()).unwrap()
        })
        .collect::<Vec<_>>();

    let message_count = decoded_tasks
        .iter()
        .map(|task| task.messages.len())
        .sum::<usize>();
    assert_eq!(message_count, 11);

    let request = build_passive_suggestions_request(decoded_tasks);
    assert!(!request.encode_to_vec().is_empty());
}

#[test]
fn seed_and_exercise_smoke_test() {
    let temp_dir = tempfile::tempdir().unwrap();
    let database = temp_dir.path().join("app-4525-repro.sqlite");

    seed(SeedArgs {
        database: database_override(database.clone()),
        conversations: 1,
        tasks_per_conversation: 2,
        large_results_per_task: 1,
        files_per_result: 1,
        file_bytes: 128,
        include_server_token: true,
        replace_existing_repro_rows: true,
        exercise_after_seed: false,
        passive_request_output: None,
    })
    .unwrap();

    let stats = exercise(ExerciseArgs {
        database: database_override(database),
        conversation_id: None,
        passive_request_output: None,
    })
    .unwrap();

    assert_eq!(stats.conversations, 1);
    assert_eq!(stats.tasks, 2);
    assert!(stats.encoded_task_bytes > 0);
    assert!(stats.passive_request_bytes > 0);
}

#[test]
fn default_database_path_targets_warplocal() {
    let database = resolve_database_path(&DatabaseArgs {
        profile: None,
        database_override: None,
        allow_non_warplocal_database: false,
    })
    .unwrap();

    assert!(database.ends_with("Library/Application Support/dev.warp.Warp-Local/warp.sqlite"));
}

#[test]
fn profile_database_path_targets_warplocal_profile() {
    let database = resolve_database_path(&DatabaseArgs {
        profile: Some("app-4525".to_string()),
        database_override: None,
        allow_non_warplocal_database: false,
    })
    .unwrap();

    assert!(
        database.ends_with("Library/Application Support/dev.warp.Warp-Local-app-4525/warp.sqlite")
    );
}

#[test]
fn database_override_rejects_non_warplocal_channels() {
    let database = PathBuf::from(
        "/Users/example/Library/Application Support/dev.warp.Warp-Preview/warp.sqlite",
    );

    let error = resolve_database_path(&DatabaseArgs {
        profile: None,
        database_override: Some(database),
        allow_non_warplocal_database: false,
    })
    .unwrap_err()
    .to_string();

    assert!(error.contains("non-WarpLocal channel database"));
}

#[test]
fn database_override_allows_warplocal_profiles() {
    let database = PathBuf::from(
        "/Users/example/Library/Application Support/dev.warp.Warp-Local-app-4525/warp.sqlite",
    );

    let resolved_database = resolve_database_path(&DatabaseArgs {
        profile: None,
        database_override: Some(database.clone()),
        allow_non_warplocal_database: false,
    })
    .unwrap();

    assert_eq!(resolved_database, database);
}

fn database_override(database: PathBuf) -> DatabaseArgs {
    DatabaseArgs {
        profile: None,
        database_override: Some(database),
        allow_non_warplocal_database: true,
    }
}
