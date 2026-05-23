use std::ffi::OsString;

use clap::Parser as _;
use clap_complete::aot::Shell;
use local_control::protocol::{ControlError, ErrorCode};
use serde_json::json;
use serial_test::serial;

use super::*;

const DISCOVERY_DIR_ENV: &str = "WARP_LOCAL_CONTROL_DISCOVERY_DIR";

fn set_discovery_dir(path: &std::path::Path) -> Option<OsString> {
    let previous = std::env::var_os(DISCOVERY_DIR_ENV);
    unsafe { std::env::set_var(DISCOVERY_DIR_ENV, path) };
    previous
}

fn restore_discovery_dir(previous: Option<OsString>) {
    match previous {
        Some(value) => unsafe { std::env::set_var(DISCOVERY_DIR_ENV, value) },
        None => unsafe { std::env::remove_var(DISCOVERY_DIR_ENV) },
    }
}
#[test]
fn parses_first_slice_tab_create() {
    let args = ControlArgs::try_parse_from(["warpctrl", "tab", "create", "--instance", "inst_123"])
        .expect("tab create parses");
    let ControlCommand::Tab(TabCommand::Create(target)) = args.command else {
        panic!("expected tab create command");
    };
    assert_eq!(target.instance.as_deref(), Some("inst_123"));
}
#[test]
fn parses_target_selector_aliases() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "tab",
        "create",
        "--window-id",
        "window-1",
        "--tab-index",
        "2",
        "--pane",
        "active",
    ])
    .expect("selector aliases parse");
    let ControlCommand::Tab(TabCommand::Create(target)) = args.command else {
        panic!("expected tab create command");
    };
    assert_eq!(
        target.target_selector(),
        TargetSelector {
            window: Some(WindowTarget::Id {
                id: WindowSelector("window-1".to_owned()),
            }),
            tab: Some(TabTarget::Index { index: 2 }),
            pane: Some(PaneTarget::Active),
            ..TargetSelector::default()
        }
    );
}
#[test]
fn parses_generic_target_selectors() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "tab",
        "create",
        "--window",
        "title:Main",
        "--tab",
        "id:tab-1",
        "--pane",
        "index:3",
    ])
    .expect("generic selectors parse");
    let ControlCommand::Tab(TabCommand::Create(target)) = args.command else {
        panic!("expected tab create command");
    };
    assert_eq!(
        target.target_selector(),
        TargetSelector {
            window: Some(WindowTarget::Title {
                title: "Main".to_owned(),
            }),
            tab: Some(TabTarget::Id {
                id: TabSelector("tab-1".to_owned()),
            }),
            pane: Some(PaneTarget::Index { index: 3 }),
            ..TargetSelector::default()
        }
    );
}
#[test]
fn parses_read_only_target_selector_families() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "session",
        "list",
        "--session-id",
        "session-1",
        "--block-index",
        "4",
        "--file-path",
        "/tmp/example.txt",
        "--drive-id",
        "workflow:workflow-1",
    ])
    .expect("read-only selector families parse");
    let ControlCommand::Session(SessionCommand::List(target)) = args.command else {
        panic!("expected session list command");
    };
    assert_eq!(
        target.target_selector(),
        TargetSelector {
            window: None,
            tab: None,
            pane: None,
            session: Some(SessionTarget::Id {
                id: SessionSelector("session-1".to_owned()),
            }),
            block: Some(BlockTarget::Index { index: 4 }),
            file: Some(FileTarget::Path {
                path: "/tmp/example.txt".to_owned(),
            }),
            drive: Some(DriveTarget::Id {
                object_type: local_control::DriveObjectType::Workflow,
                id: DriveObjectSelector("workflow-1".to_owned()),
            }),
        }
    );
}
#[test]
fn parses_mutating_target_selector_families() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "tab",
        "rename",
        "--window-id",
        "window-1",
        "--tab-id",
        "tab-1",
        "Build",
    ])
    .expect("mutating selector families parse");
    let ControlCommand::Tab(TabCommand::Rename(rename)) = args.command else {
        panic!("expected tab rename command");
    };
    assert_eq!(rename.title.as_deref(), Some("Build"));
    assert_eq!(
        rename.target.target_selector(),
        TargetSelector {
            window: Some(WindowTarget::Id {
                id: WindowSelector("window-1".to_owned()),
            }),
            tab: Some(TabTarget::Id {
                id: TabSelector("tab-1".to_owned()),
            }),
            ..TargetSelector::default()
        }
    );
}
#[test]
fn rejects_conflicting_target_selectors() {
    assert!(
        ControlArgs::try_parse_from([
            "warpctrl",
            "tab",
            "create",
            "--window",
            "active",
            "--window-id",
            "window-1",
        ])
        .is_err()
    );
    assert!(
        ControlArgs::try_parse_from([
            "warpctrl",
            "tab",
            "create",
            "--tab-id",
            "tab-1",
            "--tab-title",
            "Main",
        ])
        .is_err()
    );
    assert!(
        ControlArgs::try_parse_from([
            "warpctrl",
            "tab",
            "create",
            "--pane",
            "active",
            "--pane-index",
            "0",
        ])
        .is_err()
    );
}
#[test]
fn app_metadata_commands_reject_app_targets() {
    let args = ControlArgs::try_parse_from(["warpctrl", "app", "ping", "--tab-id", "tab-1"])
        .expect("app ping parses");
    let error = run_inner(args).expect_err("app ping target is rejected");
    assert_eq!(error.code, ErrorCode::InvalidSelector);
}

#[test]
fn parses_first_slice_instance_list() {
    let args = ControlArgs::try_parse_from(["warpctrl", "instance", "list"])
        .expect("instance list parses");
    assert!(matches!(
        args.command,
        ControlCommand::Instance(InstanceCommand::List)
    ));
}

#[test]
fn parses_first_slice_app_smoke_metadata_commands() {
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "ping"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "version"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "active"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "inspect"]).is_ok());
}

#[test]
fn parses_completion_generation_command() {
    let args = ControlArgs::try_parse_from(["warpctrl", "completions", "bash"])
        .expect("completions parses");
    assert!(matches!(
        args.command,
        ControlCommand::Completions {
            shell: Some(Shell::Bash)
        }
    ));
}

#[test]
fn parses_read_only_contract_commands() {
    let commands = [
        vec!["warpctrl", "action", "list"],
        vec!["warpctrl", "action", "get", "tab.create"],
        vec!["warpctrl", "window", "list"],
        vec!["warpctrl", "tab", "list"],
        vec!["warpctrl", "pane", "list"],
        vec!["warpctrl", "session", "list"],
        vec!["warpctrl", "block", "list", "--limit", "10"],
        vec!["warpctrl", "block", "get", "block_123"],
        vec!["warpctrl", "input", "get"],
        vec!["warpctrl", "history", "list", "--limit", "20"],
        vec!["warpctrl", "theme", "list"],
        vec!["warpctrl", "appearance", "get"],
        vec!["warpctrl", "setting", "list"],
        vec!["warpctrl", "setting", "get", "appearance.theme"],
        vec!["warpctrl", "file", "list"],
        vec!["warpctrl", "project", "active"],
        vec!["warpctrl", "project", "list"],
        vec!["warpctrl", "drive", "list", "--type", "workflow"],
        vec![
            "warpctrl",
            "drive",
            "get",
            "--type",
            "notebook",
            "notebook_123",
        ],
    ];
    for command in commands {
        ControlArgs::try_parse_from(command).expect("read-only command parses");
    }
}

#[test]
fn parses_mutating_contract_commands() {
    let commands = [
        vec!["warpctrl", "app", "focus"],
        vec![
            "warpctrl",
            "app",
            "settings-open",
            "--query",
            "theme",
            "--page",
            "appearance",
        ],
        vec!["warpctrl", "app", "command-palette-open", "--query", "git"],
        vec!["warpctrl", "app", "command-search-open", "--query", "build"],
        vec!["warpctrl", "app", "warp-drive-open"],
        vec!["warpctrl", "app", "warp-drive-toggle"],
        vec!["warpctrl", "app", "resource-center-toggle"],
        vec!["warpctrl", "app", "ai-assistant-toggle"],
        vec!["warpctrl", "app", "code-review-toggle"],
        vec!["warpctrl", "app", "vertical-tabs-toggle"],
        vec!["warpctrl", "window", "create", "--profile", "Default"],
        vec!["warpctrl", "window", "focus"],
        vec!["warpctrl", "window", "close", "--force"],
        vec!["warpctrl", "tab", "activate"],
        vec!["warpctrl", "tab", "previous"],
        vec!["warpctrl", "tab", "next"],
        vec!["warpctrl", "tab", "last"],
        vec!["warpctrl", "tab", "move", "--direction", "left"],
        vec!["warpctrl", "tab", "rename", "build"],
        vec!["warpctrl", "tab", "rename", "--reset"],
        vec!["warpctrl", "tab", "close", "--scope", "others", "--force"],
        vec!["warpctrl", "pane", "split", "--direction", "right"],
        vec!["warpctrl", "pane", "focus"],
        vec!["warpctrl", "pane", "navigate", "--direction", "down"],
        vec!["warpctrl", "pane", "close", "--force"],
        vec!["warpctrl", "pane", "maximize", "--enabled", "true"],
        vec![
            "warpctrl",
            "pane",
            "resize",
            "--direction",
            "left",
            "--amount",
            "10",
        ],
        vec!["warpctrl", "pane", "previous-session"],
        vec!["warpctrl", "pane", "next-session"],
        vec!["warpctrl", "input", "insert", "cargo check", "--replace"],
        vec!["warpctrl", "input", "replace", "cargo test"],
        vec!["warpctrl", "input", "clear"],
        vec!["warpctrl", "input", "mode", "agent"],
        vec!["warpctrl", "input", "run", "cargo check"],
        vec!["warpctrl", "theme", "set", "Warp Dark"],
        vec!["warpctrl", "appearance", "set", "--theme", "Warp Dark"],
        vec!["warpctrl", "appearance", "font-size", "increase"],
        vec!["warpctrl", "appearance", "zoom", "set", "--value", "120"],
        vec![
            "warpctrl",
            "setting",
            "set",
            "appearance.theme",
            "Warp Dark",
        ],
        vec!["warpctrl", "setting", "toggle", "appearance.follow_system"],
        vec!["warpctrl", "file", "open", "src/main.rs", "--line", "12"],
        vec![
            "warpctrl",
            "file",
            "write",
            "notes.txt",
            "hello",
            "--create",
        ],
        vec!["warpctrl", "file", "delete", "notes.txt", "--recursive"],
        vec![
            "warpctrl",
            "drive",
            "create",
            "--type",
            "workflow",
            "build",
            "{\"command\":\"cargo check\"}",
        ],
        vec![
            "warpctrl",
            "drive",
            "update",
            "--type",
            "notebook",
            "notebook_123",
            "{\"title\":\"notes\"}",
        ],
        vec![
            "warpctrl",
            "drive",
            "delete",
            "--type",
            "prompt",
            "prompt_123",
        ],
        vec![
            "warpctrl",
            "drive",
            "run",
            "--type",
            "workflow",
            "workflow_123",
        ],
        vec![
            "warpctrl",
            "drive",
            "insert",
            "--type",
            "notebook",
            "notebook_123",
        ],
    ];
    for command in commands {
        ControlArgs::try_parse_from(command).expect("mutating contract command parses");
    }
}

#[test]
fn parses_mutating_command_values_into_typed_args() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "pane",
        "resize",
        "--direction",
        "up",
        "--amount",
        "8",
    ])
    .expect("pane resize parses");
    let ControlCommand::Pane(PaneCommand::Resize(resize)) = args.command else {
        panic!("expected pane resize command");
    };
    assert!(matches!(resize.direction, PaneDirectionArg::Up));
    assert_eq!(resize.amount, Some(8));

    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "drive",
        "create",
        "--type",
        "workflow",
        "build",
        "{\"command\":\"cargo check\"}",
    ])
    .expect("drive create parses");
    let ControlCommand::Drive(DriveCommand::Create(create)) = args.command else {
        panic!("expected drive create command");
    };
    assert!(matches!(create.object_type, DriveObjectTypeArg::Workflow));
    assert_eq!(create.name, "build");
    assert_eq!(create.content, "{\"command\":\"cargo check\"}");
}

#[test]
fn generated_bash_completions_include_first_slice_commands() {
    let completions =
        generate_completion_string(Shell::Bash).expect("bash completions render to UTF-8");
    assert!(completions.contains("instance"));
    assert!(completions.contains("tab"));
    assert!(completions.contains("window"));
    assert!(completions.contains("setting"));
    assert!(completions.contains("project"));
    assert!(completions.contains("drive"));
    assert!(completions.contains("completions"));
    assert!(completions.contains("insert"));
    assert!(completions.contains("run"));
    assert!(completions.contains("write"));
    assert!(completions.contains("delete"));
}

#[test]
fn structured_error_output_uses_stable_error_code() {
    let error = ControlError::new(ErrorCode::NoInstance, "no local Warp control instances");
    let value = serde_json::to_value(ErrorSummary {
        ok: false,
        error: &error,
    })
    .expect("error summary serializes");
    assert_eq!(value["ok"], json!(false));
    assert_eq!(value["error"]["code"], json!("no_instance"));
    assert_eq!(
        value["error"]["message"],
        json!("no local Warp control instances")
    );
}

#[test]
#[serial]
fn tab_create_without_discovery_records_reports_no_instance() {
    let dir = std::env::temp_dir().join(format!(
        "warpctrl-empty-discovery-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).expect("temp discovery dir is created");
    let previous = set_discovery_dir(&dir);
    let args =
        ControlArgs::try_parse_from(["warpctrl", "--output-format", "json", "tab", "create"])
            .expect("tab create parses");
    let error = run_inner(args).expect_err("missing instance is rejected");
    restore_discovery_dir(previous);
    assert_eq!(error.code, ErrorCode::NoInstance);
}
