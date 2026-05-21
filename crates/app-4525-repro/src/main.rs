use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_query,
    sql_types::{Binary, Text},
    sqlite::SqliteConnection,
};
use instant::Instant;
use prost::Message as _;
use serde_json::json;
use uuid::Uuid;
use warp_multi_agent_api as api;

const REPRO_MARKER: &str = "APP-4525";
const WARP_LOCAL_APP_DATA_DIR_NAME: &str = "dev.warp.Warp-Local";
const WARP_SQLITE_FILE_NAME: &str = "warp.sqlite";
const NON_WARP_LOCAL_APP_DATA_DIR_NAMES: &[&str] = &[
    "dev.warp.Warp-Stable",
    "dev.warp.Warp-Dev",
    "dev.warp.Warp-Preview",
    "dev.warp.WarpOss",
];
const LEGACY_STABLE_APP_DATA_DIR_NAME: &str = "dev.warp.Warp";

#[derive(Parser)]
#[command(
    about = "Seed and exercise large persisted agent task data for APP-4525 memory repros.",
    long_about = "Creates synthetic agent_conversations/agent_tasks rows with large tool-result payloads, then optionally decodes, clones, and re-encodes them to mimic the expensive restoration and passive-suggestion paths from APP-4525."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Seed(SeedArgs),
    Exercise(ExerciseArgs),
}

#[derive(Parser)]
struct SeedArgs {
    #[command(flatten)]
    database: DatabaseArgs,
    #[arg(long, default_value_t = 1)]
    conversations: usize,
    #[arg(long, default_value_t = 8)]
    tasks_per_conversation: usize,
    #[arg(long, default_value_t = 2)]
    large_results_per_task: usize,
    #[arg(long, default_value_t = 4)]
    files_per_result: usize,
    #[arg(long, default_value = "1MiB", value_parser = parse_bytes)]
    file_bytes: usize,
    #[arg(long)]
    include_server_token: bool,
    #[arg(long)]
    replace_existing_repro_rows: bool,
    #[arg(long)]
    exercise_after_seed: bool,
    #[arg(long)]
    passive_request_output: Option<PathBuf>,
}

#[derive(Parser)]
struct ExerciseArgs {
    #[command(flatten)]
    database: DatabaseArgs,
    #[arg(long)]
    conversation_id: Option<String>,
    #[arg(long)]
    passive_request_output: Option<PathBuf>,
}
#[derive(Clone, Parser)]
struct DatabaseArgs {
    #[arg(
        long,
        value_name = "PROFILE",
        conflicts_with = "database_override",
        help = "Target a WarpLocal development profile, mapping to dev.warp.Warp-Local-<profile>/warp.sqlite"
    )]
    profile: Option<String>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Override the database path. Refuses non-WarpLocal paths unless --allow-non-warplocal-database is also passed"
    )]
    database_override: Option<PathBuf>,
    #[arg(
        long,
        requires = "database_override",
        help = "Allow --database-override to point outside WarpLocal app data"
    )]
    allow_non_warplocal_database: bool,
}

#[derive(QueryableByName)]
struct TaskRow {
    #[diesel(sql_type = Text)]
    conversation_id: String,
    #[diesel(sql_type = Text)]
    task_id: String,
    #[diesel(sql_type = Binary)]
    task: Vec<u8>,
}

#[derive(Default)]
struct SeedStats {
    conversations: usize,
    tasks: usize,
    messages: usize,
    encoded_task_bytes: usize,
}

struct ExerciseStats {
    conversations: usize,
    tasks: usize,
    messages: usize,
    encoded_task_bytes: usize,
    passive_request_bytes: usize,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Seed(args) => seed(args),
        Command::Exercise(args) => exercise(args).map(|_| ()),
    }
}

fn seed(args: SeedArgs) -> Result<()> {
    if args.tasks_per_conversation == 0 {
        return Err(anyhow!("--tasks-per-conversation must be at least 1"));
    }
    if args.large_results_per_task == 0 {
        return Err(anyhow!("--large-results-per-task must be at least 1"));
    }
    if args.files_per_result == 0 {
        return Err(anyhow!("--files-per-result must be at least 1"));
    }

    let database = resolve_database_path(&args.database)?;
    let mut conn = connect(&database)?;
    ensure_agent_tables(&mut conn)?;

    if args.replace_existing_repro_rows {
        delete_existing_repro_rows(&mut conn)?;
    }

    let started_at = Instant::now();
    let mut stats = SeedStats::default();
    for conversation_index in 0..args.conversations {
        let conversation_id = Uuid::new_v4().to_string();
        let server_conversation_token = args
            .include_server_token
            .then(|| format!("app-4525-repro-token-{conversation_index}"));
        let conversation_data = json!({
            "server_conversation_token": server_conversation_token,
            "repro_marker": REPRO_MARKER,
            "repro_description": "Synthetic large task payloads for APP-4525",
            "root_task_is_optimistic": false,
        })
        .to_string();

        sql_query(
            "INSERT OR REPLACE INTO agent_conversations (conversation_id, conversation_data) VALUES (?, ?)",
        )
        .bind::<Text, _>(&conversation_id)
        .bind::<Text, _>(&conversation_data)
        .execute(&mut conn)
        .context("failed to upsert agent_conversations row")?;

        let tasks = build_conversation_tasks(
            conversation_index,
            args.tasks_per_conversation,
            args.large_results_per_task,
            args.files_per_result,
            args.file_bytes,
        );

        for task in tasks {
            let task_binary = task.encode_to_vec();
            stats.tasks += 1;
            stats.messages += task.messages.len();
            stats.encoded_task_bytes += task_binary.len();

            sql_query(
                "INSERT OR REPLACE INTO agent_tasks (conversation_id, task_id, task) VALUES (?, ?, ?)",
            )
            .bind::<Text, _>(&conversation_id)
            .bind::<Text, _>(&task.id)
            .bind::<Binary, _>(&task_binary)
            .execute(&mut conn)
            .with_context(|| format!("failed to upsert agent_tasks row for {}", task.id))?;
        }

        stats.conversations += 1;
    }

    println!(
        "Seeded {} conversations, {} tasks, {} messages, {} encoded task bytes in {:?}",
        stats.conversations,
        stats.tasks,
        stats.messages,
        format_bytes(stats.encoded_task_bytes),
        started_at.elapsed()
    );
    println!("Database: {}", database.display());

    if args.exercise_after_seed {
        let exercise_args = ExerciseArgs {
            database: args.database,
            conversation_id: None,
            passive_request_output: args.passive_request_output,
        };
        exercise(exercise_args)?;
    }

    Ok(())
}

fn exercise(args: ExerciseArgs) -> Result<ExerciseStats> {
    let database = resolve_database_path(&args.database)?;
    let mut conn = connect(&database)?;
    let before_rss = max_resident_set_size_bytes();
    let query_started_at = Instant::now();
    let rows = read_task_rows(&mut conn, args.conversation_id.as_deref())?;
    let query_elapsed = query_started_at.elapsed();
    let after_query_rss = max_resident_set_size_bytes();

    let encoded_task_bytes = rows.iter().map(|row| row.task.len()).sum();
    let decode_started_at = Instant::now();
    let mut tasks_by_conversation = BTreeMap::<String, Vec<api::Task>>::new();
    let mut messages = 0;
    for row in rows {
        let task = api::Task::decode(row.task.as_slice())
            .with_context(|| format!("failed to decode task {}", row.task_id))?;
        messages += task.messages.len();
        tasks_by_conversation
            .entry(row.conversation_id)
            .or_default()
            .push(task);
    }
    let decode_elapsed = decode_started_at.elapsed();
    let after_decode_rss = max_resident_set_size_bytes();

    let clone_started_at = Instant::now();
    let cloned_tasks = tasks_by_conversation
        .values()
        .flat_map(|tasks| tasks.iter().cloned())
        .collect::<Vec<_>>();
    let clone_elapsed = clone_started_at.elapsed();
    let after_clone_rss = max_resident_set_size_bytes();

    let encode_started_at = Instant::now();
    let passive_request = build_passive_suggestions_request(cloned_tasks);
    let passive_request_bytes = passive_request.encode_to_vec();
    let encode_elapsed = encode_started_at.elapsed();
    let after_encode_rss = max_resident_set_size_bytes();

    if let Some(output) = args.passive_request_output {
        fs::write(&output, &passive_request_bytes)
            .with_context(|| format!("failed to write {}", output.display()))?;
        println!(
            "Wrote passive suggestion request protobuf: {}",
            output.display()
        );
    }

    let stats = ExerciseStats {
        conversations: tasks_by_conversation.len(),
        tasks: tasks_by_conversation.values().map(Vec::len).sum(),
        messages,
        encoded_task_bytes,
        passive_request_bytes: passive_request_bytes.len(),
    };

    println!("Exercised APP-4525 fixture from {}", database.display());
    println!("Conversations: {}", stats.conversations);
    println!("Tasks: {}", stats.tasks);
    println!("Messages: {}", stats.messages);
    println!(
        "Persisted encoded task bytes: {}",
        format_bytes(stats.encoded_task_bytes)
    );
    println!(
        "Passive request encoded bytes: {}",
        format_bytes(stats.passive_request_bytes)
    );
    println!("Query time: {query_elapsed:?}");
    println!("Decode time: {decode_elapsed:?}");
    println!("Clone time: {clone_elapsed:?}");
    println!("Encode time: {encode_elapsed:?}");
    print_rss_delta("After query", before_rss, after_query_rss);
    print_rss_delta("After decode", before_rss, after_decode_rss);
    print_rss_delta("After clone", before_rss, after_clone_rss);
    print_rss_delta("After encode", before_rss, after_encode_rss);

    Ok(stats)
}

fn resolve_database_path(args: &DatabaseArgs) -> Result<PathBuf> {
    if let Some(database_override) = &args.database_override {
        if !args.allow_non_warplocal_database {
            ensure_warplocal_database_path(database_override)?;
        }
        return Ok(database_override.clone());
    }

    warplocal_database_path(args.profile.as_deref())
}

fn warplocal_database_path(profile: Option<&str>) -> Result<PathBuf> {
    let mut app_data_dir_name = WARP_LOCAL_APP_DATA_DIR_NAME.to_string();
    if let Some(profile) = profile {
        validate_profile_name(profile)?;
        app_data_dir_name.push('-');
        app_data_dir_name.push_str(profile);
    }

    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("failed to determine home directory"))?;
    Ok(home_dir
        .join("Library")
        .join("Application Support")
        .join(app_data_dir_name)
        .join(WARP_SQLITE_FILE_NAME))
}

fn validate_profile_name(profile: &str) -> Result<()> {
    if profile.is_empty() {
        return Err(anyhow!("--profile must not be empty"));
    }
    if profile.contains('/') || profile.contains('\\') {
        return Err(anyhow!(
            "--profile must be a profile name, not a path: {profile}"
        ));
    }
    Ok(())
}

fn ensure_warplocal_database_path(database: &Path) -> Result<()> {
    if is_warplocal_database_path(database) {
        return Ok(());
    }

    if is_known_non_warplocal_database_path(database) {
        return Err(anyhow!(
            "refusing to use APP-4525 repro data with a non-WarpLocal channel database: {}. \
             Use WarpLocal's default path, pass --profile for a WarpLocal development profile, \
             or pass --allow-non-warplocal-database if this override is intentional.",
            database.display()
        ));
    }

    Err(anyhow!(
        "--database-override must point inside dev.warp.Warp-Local app data unless \
         --allow-non-warplocal-database is passed: {}",
        database.display()
    ))
}

fn is_warplocal_database_path(database: &Path) -> bool {
    path_has_app_data_component(database, WARP_LOCAL_APP_DATA_DIR_NAME)
}

fn is_known_non_warplocal_database_path(database: &Path) -> bool {
    path_has_exact_component(database, LEGACY_STABLE_APP_DATA_DIR_NAME)
        || NON_WARP_LOCAL_APP_DATA_DIR_NAMES
            .iter()
            .any(|app_data_dir_name| path_has_app_data_component(database, app_data_dir_name))
}

fn path_has_app_data_component(path: &Path, app_data_dir_name: &str) -> bool {
    path.components().any(|component| {
        let component = component.as_os_str().to_string_lossy();
        component == app_data_dir_name
            || component
                .strip_prefix(app_data_dir_name)
                .is_some_and(|suffix| suffix.starts_with('-'))
    })
}

fn path_has_exact_component(path: &Path, expected_component: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str() == expected_component)
}

fn connect(database: &Path) -> Result<SqliteConnection> {
    if let Some(parent) = database.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let database = database
        .to_str()
        .ok_or_else(|| anyhow!("database path is not valid UTF-8: {}", database.display()))?;

    SqliteConnection::establish(database)
        .with_context(|| format!("failed to connect to sqlite database {database}"))
}

fn ensure_agent_tables(conn: &mut SqliteConnection) -> Result<()> {
    conn.batch_execute(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS agent_conversations (
            id INTEGER PRIMARY KEY NOT NULL,
            conversation_id TEXT NOT NULL,
            active_task_id TEXT,
            conversation_data TEXT NOT NULL,
            last_modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE UNIQUE INDEX IF NOT EXISTS ux_agent_conversations_conversation_id
            ON agent_conversations (conversation_id);

        CREATE TABLE IF NOT EXISTS agent_tasks (
            id INTEGER PRIMARY KEY NOT NULL,
            conversation_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            task BLOB NOT NULL,
            last_modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (conversation_id) REFERENCES agent_conversations (conversation_id)
        );

        CREATE UNIQUE INDEX IF NOT EXISTS ux_agent_tasks_task_id
            ON agent_tasks (task_id);
        "#,
    )
    .context("failed to create agent persistence tables")
}

fn delete_existing_repro_rows(conn: &mut SqliteConnection) -> Result<()> {
    sql_query(
        "DELETE FROM agent_tasks WHERE conversation_id IN (
            SELECT conversation_id FROM agent_conversations WHERE conversation_data LIKE ?
        )",
    )
    .bind::<Text, _>(format!("%\"repro_marker\":\"{REPRO_MARKER}\"%"))
    .execute(conn)
    .context("failed to delete existing repro task rows")?;

    sql_query("DELETE FROM agent_conversations WHERE conversation_data LIKE ?")
        .bind::<Text, _>(format!("%\"repro_marker\":\"{REPRO_MARKER}\"%"))
        .execute(conn)
        .context("failed to delete existing repro conversation rows")?;

    Ok(())
}

fn read_task_rows(
    conn: &mut SqliteConnection,
    conversation_id: Option<&str>,
) -> Result<Vec<TaskRow>> {
    if let Some(conversation_id) = conversation_id {
        return sql_query(
            "SELECT conversation_id, task_id, task FROM agent_tasks WHERE conversation_id = ? ORDER BY id ASC",
        )
        .bind::<Text, _>(conversation_id)
        .load(conn)
        .context("failed to load task rows for conversation");
    }

    sql_query(
        "SELECT agent_tasks.conversation_id, agent_tasks.task_id, agent_tasks.task
         FROM agent_tasks
         INNER JOIN agent_conversations
            ON agent_tasks.conversation_id = agent_conversations.conversation_id
         WHERE agent_conversations.conversation_data LIKE ?
         ORDER BY agent_conversations.id ASC, agent_tasks.id ASC",
    )
    .bind::<Text, _>(format!("%\"repro_marker\":\"{REPRO_MARKER}\"%"))
    .load(conn)
    .context("failed to load repro task rows")
}

fn build_conversation_tasks(
    conversation_index: usize,
    tasks_per_conversation: usize,
    large_results_per_task: usize,
    files_per_result: usize,
    file_bytes: usize,
) -> Vec<api::Task> {
    let root_task_id = task_id(conversation_index, 0);
    let mut root_messages = vec![
        user_query_message(
            conversation_index,
            0,
            &root_task_id,
            "Restore a large multi-agent task history with full file context.",
        ),
        agent_output_message(
            conversation_index,
            0,
            &root_task_id,
            "Starting child agents.",
        ),
    ];

    for task_index in 1..tasks_per_conversation {
        let child_task_id = task_id(conversation_index, task_index);
        root_messages.push(subagent_tool_call_message(
            conversation_index,
            task_index,
            &root_task_id,
            &child_task_id,
        ));
    }
    root_messages.extend(large_tool_result_messages(
        conversation_index,
        0,
        &root_task_id,
        large_results_per_task,
        files_per_result,
        file_bytes,
    ));

    let mut tasks = vec![api::Task {
        id: root_task_id.clone(),
        messages: root_messages,
        dependencies: None,
        description: "APP-4525 repro root task".to_string(),
        summary: String::new(),
        server_data: String::new(),
    }];

    for task_index in 1..tasks_per_conversation {
        let child_task_id = task_id(conversation_index, task_index);
        let mut messages = vec![
            agent_output_message(
                conversation_index,
                task_index,
                &child_task_id,
                "Child agent read a large amount of file context.",
            ),
            run_shell_command_result_message(
                conversation_index,
                task_index,
                &child_task_id,
                file_bytes,
            ),
        ];
        messages.extend(large_tool_result_messages(
            conversation_index,
            task_index,
            &child_task_id,
            large_results_per_task,
            files_per_result,
            file_bytes,
        ));

        tasks.push(api::Task {
            id: child_task_id,
            messages,
            dependencies: Some(api::task::Dependencies {
                parent_task_id: root_task_id.clone(),
            }),
            description: format!("APP-4525 repro child task {task_index}"),
            summary: String::new(),
            server_data: String::new(),
        });
    }

    tasks
}

fn large_tool_result_messages(
    conversation_index: usize,
    task_index: usize,
    task_id: &str,
    large_results_per_task: usize,
    files_per_result: usize,
    file_bytes: usize,
) -> Vec<api::Message> {
    (0..large_results_per_task)
        .map(|result_index| {
            read_files_result_message(
                conversation_index,
                task_index,
                result_index,
                task_id,
                files_per_result,
                file_bytes,
            )
        })
        .collect()
}

fn user_query_message(
    conversation_index: usize,
    task_index: usize,
    task_id: &str,
    query: &str,
) -> api::Message {
    api::Message {
        id: message_id(conversation_index, task_index, "user", 0),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::UserQuery(api::message::UserQuery {
            query: query.to_string(),
            context: None,
            referenced_attachments: HashMap::new(),
            mode: None,
            intended_agent: Default::default(),
        })),
        request_id: request_id(conversation_index, task_index, 0),
        timestamp: None,
    }
}

fn agent_output_message(
    conversation_index: usize,
    task_index: usize,
    task_id: &str,
    text: &str,
) -> api::Message {
    api::Message {
        id: message_id(conversation_index, task_index, "agent", 0),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: text.to_string(),
            },
        )),
        request_id: request_id(conversation_index, task_index, 0),
        timestamp: None,
    }
}

fn subagent_tool_call_message(
    conversation_index: usize,
    task_index: usize,
    task_id: &str,
    child_task_id: &str,
) -> api::Message {
    api::Message {
        id: message_id(conversation_index, task_index, "subagent-call", 0),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: format!("subagent-call-{conversation_index}-{task_index}"),
            tool: Some(api::message::tool_call::Tool::Subagent(
                api::message::tool_call::Subagent {
                    task_id: child_task_id.to_string(),
                    payload: String::new(),
                    metadata: None,
                },
            )),
        })),
        request_id: request_id(conversation_index, 0, 0),
        timestamp: None,
    }
}

fn read_files_result_message(
    conversation_index: usize,
    task_index: usize,
    result_index: usize,
    task_id: &str,
    files_per_result: usize,
    file_bytes: usize,
) -> api::Message {
    let files = (0..files_per_result)
        .map(|file_index| api::AnyFileContent {
            content: Some(api::any_file_content::Content::TextContent(
                api::FileContent {
                    file_path: format!(
                        "/tmp/app-4525/{conversation_index}/{task_index}/{result_index}/{file_index}.txt"
                    ),
                    content: large_text(
                        conversation_index,
                        task_index,
                        result_index,
                        file_index,
                        file_bytes,
                    ),
                    line_range: None,
                },
            )),
        })
        .collect();

    api::Message {
        id: message_id(
            conversation_index,
            task_index,
            "read-files-result",
            result_index,
        ),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::ToolCallResult(
            api::message::ToolCallResult {
                tool_call_id: format!(
                    "read-files-{conversation_index}-{task_index}-{result_index}"
                ),
                context: None,
                result: Some(api::message::tool_call_result::Result::ReadFiles(
                    api::ReadFilesResult {
                        result: Some(api::read_files_result::Result::AnyFilesSuccess(
                            api::read_files_result::AnyFilesSuccess { files },
                        )),
                    },
                )),
            },
        )),
        request_id: request_id(conversation_index, task_index, result_index + 1),
        timestamp: None,
    }
}

fn run_shell_command_result_message(
    conversation_index: usize,
    task_index: usize,
    task_id: &str,
    output_bytes: usize,
) -> api::Message {
    api::Message {
        id: message_id(conversation_index, task_index, "shell-result", 0),
        task_id: task_id.to_string(),
        server_message_data: String::new(),
        citations: vec![],
        message: Some(api::message::Message::ToolCallResult(
            api::message::ToolCallResult {
                tool_call_id: format!("shell-{conversation_index}-{task_index}"),
                context: None,
                result: Some(api::message::tool_call_result::Result::RunShellCommand(
                    api::RunShellCommandResult {
                        command: "cat very-large-output.txt".to_string(),
                        result: Some(api::run_shell_command_result::Result::CommandFinished(
                            api::ShellCommandFinished {
                                command_id: format!("command-{conversation_index}-{task_index}"),
                                output: large_text(
                                    conversation_index,
                                    task_index,
                                    usize::MAX,
                                    0,
                                    output_bytes,
                                ),
                                exit_code: 0,
                                start_ts: None,
                                finish_ts: None,
                            },
                        )),
                        ..Default::default()
                    },
                )),
            },
        )),
        request_id: request_id(conversation_index, task_index, 0),
        timestamp: None,
    }
}

fn build_passive_suggestions_request(tasks: Vec<api::Task>) -> api::Request {
    api::Request {
        task_context: Some(api::request::TaskContext { tasks }),
        input: Some(api::request::Input {
            context: None,
            r#type: Some(api::request::input::Type::GeneratePassiveSuggestions(
                api::request::input::GeneratePassiveSuggestions {
                    attachments: vec![],
                    trigger: Some(
                        api::request::input::generate_passive_suggestions::Trigger::AgentResponseCompleted(
                            api::request::input::generate_passive_suggestions::AgentResponseCompleted {},
                        ),
                    ),
                },
            )),
        }),
        settings: Some(api::request::Settings::default()),
        metadata: Some(api::request::Metadata {
            logging: HashMap::new(),
            conversation_id: "app-4525-repro-passive-request".to_string(),
            ambient_agent_task_id: String::new(),
            forked_from_conversation_id: String::new(),
            parent_agent_id: String::new(),
            agent_name: String::new(),
        }),
        existing_suggestions: None,
        mcp_context: None,
    }
}

fn task_id(conversation_index: usize, task_index: usize) -> String {
    format!("app-4525-repro-task-{conversation_index}-{task_index}")
}

fn message_id(
    conversation_index: usize,
    task_index: usize,
    kind: &str,
    message_index: usize,
) -> String {
    format!("app-4525-repro-message-{conversation_index}-{task_index}-{kind}-{message_index}")
}

fn request_id(conversation_index: usize, task_index: usize, request_index: usize) -> String {
    format!("app-4525-repro-request-{conversation_index}-{task_index}-{request_index}")
}

fn large_text(
    conversation_index: usize,
    task_index: usize,
    result_index: usize,
    file_index: usize,
    bytes: usize,
) -> String {
    let pattern = format!(
        "app-4525 conversation={conversation_index} task={task_index} result={result_index} file={file_index} restoring large tool payload\n"
    );
    let mut output = String::with_capacity(bytes);
    while output.len() < bytes {
        output.push_str(&pattern);
    }
    output.truncate(bytes);
    output
}

fn parse_bytes(value: &str) -> Result<usize, String> {
    let trimmed = value.trim();
    let suffix_start = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (digits, suffix) = trimmed.split_at(suffix_start);
    let amount = digits
        .parse::<usize>()
        .map_err(|error| format!("invalid byte count {value:?}: {error}"))?;
    let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        other => {
            return Err(format!(
                "unknown byte suffix {other:?}; use B, KiB, MiB, or GiB"
            ))
        }
    };
    amount
        .checked_mul(multiplier)
        .ok_or_else(|| format!("byte count {value:?} is too large"))
}

fn format_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn max_resident_set_size_bytes() -> Option<usize> {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
        if result != 0 {
            return None;
        }
        let max_rss = unsafe { usage.assume_init().ru_maxrss };
        #[cfg(target_os = "macos")]
        {
            Some(max_rss as usize)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Some(max_rss as usize * 1024)
        }
    }

    #[cfg(not(unix))]
    {
        None
    }
}

fn print_rss_delta(label: &str, before: Option<usize>, after: Option<usize>) {
    let Some(before) = before else {
        return;
    };
    let Some(after) = after else {
        return;
    };
    let delta = after.saturating_sub(before);
    println!(
        "{label} max RSS: {} (+{})",
        format_bytes(after),
        format_bytes(delta)
    );
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
