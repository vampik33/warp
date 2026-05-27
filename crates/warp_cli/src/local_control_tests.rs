use std::{collections::HashSet, ffi::OsString};

use clap::Parser as _;
use clap_complete::aot::Shell;
use local_control::protocol::{ActionKind, ControlError, ErrorCode};
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
    assert!(ControlArgs::try_parse_from(["warpctrl", "app", "focus"]).is_ok());
}

#[test]
fn parses_catalog_metadata_commands() {
    let args =
        ControlArgs::try_parse_from(["warpctrl", "action", "inspect", "surface.settings.open"])
            .expect("action inspect parses");
    let ControlCommand::Action(ActionCatalogCommand::Inspect { action }) = args.command else {
        panic!("expected action inspect command");
    };
    assert_eq!(action, "surface.settings.open");
    assert!(ControlArgs::try_parse_from(["warpctrl", "action", "list", "--stubs-only"]).is_ok());
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "capability", "list", "--implemented-only",])
            .is_ok()
    );
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "capability", "inspect", "tab.create"]).is_ok()
    );
}

#[test]
fn parses_execution_underlying_commands() {
    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "input",
        "run",
        "cargo check",
        "--instance",
        "inst_123",
    ])
    .expect("input run parses");
    let ControlCommand::Input(InputCommand::Run(input_args)) = args.command else {
        panic!("expected input run command");
    };
    assert_eq!(input_args.text, "cargo check");
    assert_eq!(input_args.target.instance.as_deref(), Some("inst_123"));

    let args = ControlArgs::try_parse_from([
        "warpctrl",
        "drive",
        "workflow",
        "run",
        "workflow_123",
        "--arg",
        "name=value",
    ])
    .expect("drive workflow run parses");
    let ControlCommand::Drive(DriveCommand::Workflow(DriveWorkflowCommand::Run(workflow_args))) =
        args.command
    else {
        panic!("expected drive workflow run command");
    };
    assert_eq!(workflow_args.id, "workflow_123");
    assert_eq!(workflow_args.args[0].name, "name");
    assert_eq!(workflow_args.args[0].value, "value");
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
fn parses_readonly_capability_and_target_commands() {
    assert!(ControlArgs::try_parse_from(["warpctrl", "instance", "inspect"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "capability", "list"]).is_ok());
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "capability", "inspect", "tab.create"]).is_ok()
    );
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "action", "inspect", "drive.inspect"]).is_ok()
    );
    assert!(ControlArgs::try_parse_from(["warpctrl", "window", "list"]).is_ok());
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "window", "inspect", "--window", "active"])
            .is_ok()
    );
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "tab", "inspect", "--tab-index", "0"]).is_ok()
    );
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "pane", "inspect", "--pane", "active"]).is_ok()
    );
    assert!(
        ControlArgs::try_parse_from(["warpctrl", "session", "inspect", "--session", "active"])
            .is_ok()
    );
    assert!(ControlArgs::try_parse_from(["warpctrl", "block", "output", "block_1"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "input", "get"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "history", "list", "--limit", "5"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "theme", "get"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "keybinding", "get", "copy"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "file", "list"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "project", "active"]).is_ok());
    assert!(ControlArgs::try_parse_from(["warpctrl", "drive", "inspect", "drive_1"]).is_ok());
}

#[test]
fn excluded_actions_are_not_allowlisted_catalog_entries() {
    let args = ControlArgs::try_parse_from(["warpctrl", "action", "inspect", "auth.api_key.set"])
        .expect("action inspect parses arbitrary action name");
    let error = run_inner(args).expect_err("excluded auth api-key action is not allowlisted");
    assert_eq!(error.code, ErrorCode::NotAllowlisted);
    let args = ControlArgs::try_parse_from(["warpctrl", "action", "inspect", "file.write"])
        .expect("action inspect parses arbitrary action name");
    let error = run_inner(args).expect_err("excluded file mutation action is not allowlisted");
    assert_eq!(error.code, ErrorCode::NotAllowlisted);
}

#[test]
fn generated_bash_completions_include_readonly_commands() {
    let completions =
        generate_completion_string(Shell::Bash).expect("bash completions render to UTF-8");
    assert!(completions.contains("instance"));
    assert!(completions.contains("action"));
    assert!(completions.contains("capability"));
    assert!(completions.contains("stubs-only"));
    assert!(completions.contains("window"));
    assert!(completions.contains("block"));
    assert!(completions.contains("input"));
    assert!(completions.contains("drive"));
    assert!(completions.contains("completions"));
}

#[test]
fn every_implemented_catalog_action_has_a_parseable_cli_example() {
    let mut covered = HashSet::new();
    for (kind, argv) in implemented_action_examples() {
        let args = ControlArgs::try_parse_from(argv)
            .unwrap_or_else(|err| panic!("{} parses: {err}", kind.as_str()));
        assert_eq!(parsed_action_kind(&args.command), Some(kind));
        covered.insert(kind);
    }
    let expected = ActionKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_implemented())
        .collect::<HashSet<_>>();
    let missing = expected
        .difference(&covered)
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "implemented catalog actions missing parser examples: {missing:?}"
    );
}

#[test]
fn generated_bash_completions_include_mutating_command_groups() {
    let completions =
        generate_completion_string(Shell::Bash).expect("bash completions render to UTF-8");
    assert!(completions.contains("surface"));
    assert!(completions.contains("command-palette"));
    assert!(completions.contains("warp-drive"));
    assert!(completions.contains("resource-center"));
    assert!(completions.contains("activate"));
    assert!(completions.contains("split"));
    assert!(completions.contains("mode"));
    assert!(completions.contains("env-var-collection"));
    assert!(completions.contains("share-to-team"));
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

fn implemented_action_examples() -> Vec<(ActionKind, Vec<&'static str>)> {
    vec![
        (
            ActionKind::InstanceList,
            vec!["warpctrl", "instance", "list"],
        ),
        (
            ActionKind::InstanceInspect,
            vec!["warpctrl", "instance", "inspect"],
        ),
        (ActionKind::AppPing, vec!["warpctrl", "app", "ping"]),
        (ActionKind::AppVersion, vec!["warpctrl", "app", "version"]),
        (ActionKind::AppActive, vec!["warpctrl", "app", "active"]),
        (ActionKind::AppFocus, vec!["warpctrl", "app", "focus"]),
        (
            ActionKind::CapabilityList,
            vec!["warpctrl", "capability", "list"],
        ),
        (
            ActionKind::CapabilityInspect,
            vec!["warpctrl", "capability", "inspect", "tab.create"],
        ),
        (ActionKind::WindowList, vec!["warpctrl", "window", "list"]),
        (
            ActionKind::WindowInspect,
            vec!["warpctrl", "window", "inspect"],
        ),
        (
            ActionKind::WindowCreate,
            vec!["warpctrl", "window", "create"],
        ),
        (ActionKind::WindowFocus, vec!["warpctrl", "window", "focus"]),
        (ActionKind::WindowClose, vec!["warpctrl", "window", "close"]),
        (ActionKind::TabList, vec!["warpctrl", "tab", "list"]),
        (ActionKind::TabInspect, vec!["warpctrl", "tab", "inspect"]),
        (ActionKind::TabCreate, vec!["warpctrl", "tab", "create"]),
        (ActionKind::TabActivate, vec!["warpctrl", "tab", "activate"]),
        (
            ActionKind::TabMove,
            vec!["warpctrl", "tab", "move", "--direction", "next"],
        ),
        (ActionKind::TabClose, vec!["warpctrl", "tab", "close"]),
        (
            ActionKind::TabRename,
            vec!["warpctrl", "tab", "rename", "docs"],
        ),
        (
            ActionKind::TabResetName,
            vec!["warpctrl", "tab", "reset-name"],
        ),
        (
            ActionKind::TabColorSet,
            vec!["warpctrl", "tab", "color", "set", "red"],
        ),
        (
            ActionKind::TabColorClear,
            vec!["warpctrl", "tab", "color", "clear"],
        ),
        (ActionKind::PaneList, vec!["warpctrl", "pane", "list"]),
        (ActionKind::PaneInspect, vec!["warpctrl", "pane", "inspect"]),
        (
            ActionKind::PaneSplit,
            vec!["warpctrl", "pane", "split", "--direction", "right"],
        ),
        (ActionKind::PaneFocus, vec!["warpctrl", "pane", "focus"]),
        (
            ActionKind::PaneNavigate,
            vec!["warpctrl", "pane", "navigate", "--direction", "next"],
        ),
        (
            ActionKind::PaneResize,
            vec![
                "warpctrl",
                "pane",
                "resize",
                "--direction",
                "right",
                "--amount",
                "4",
            ],
        ),
        (
            ActionKind::PaneMaximize,
            vec!["warpctrl", "pane", "maximize"],
        ),
        (
            ActionKind::PaneUnmaximize,
            vec!["warpctrl", "pane", "unmaximize"],
        ),
        (ActionKind::PaneClose, vec!["warpctrl", "pane", "close"]),
        (
            ActionKind::PaneRename,
            vec!["warpctrl", "pane", "rename", "server"],
        ),
        (
            ActionKind::PaneResetName,
            vec!["warpctrl", "pane", "reset-name"],
        ),
        (ActionKind::SessionList, vec!["warpctrl", "session", "list"]),
        (
            ActionKind::SessionInspect,
            vec!["warpctrl", "session", "inspect"],
        ),
        (
            ActionKind::SessionActivate,
            vec!["warpctrl", "session", "activate"],
        ),
        (
            ActionKind::SessionPrevious,
            vec!["warpctrl", "session", "previous"],
        ),
        (ActionKind::SessionNext, vec!["warpctrl", "session", "next"]),
        (
            ActionKind::SessionReopenClosed,
            vec!["warpctrl", "session", "reopen-closed"],
        ),
        (ActionKind::BlockList, vec!["warpctrl", "block", "list"]),
        (
            ActionKind::BlockInspect,
            vec!["warpctrl", "block", "inspect", "block_1"],
        ),
        (
            ActionKind::BlockOutput,
            vec!["warpctrl", "block", "output", "block_1"],
        ),
        (ActionKind::InputGet, vec!["warpctrl", "input", "get"]),
        (
            ActionKind::InputInsert,
            vec!["warpctrl", "input", "insert", "hello"],
        ),
        (
            ActionKind::InputReplace,
            vec!["warpctrl", "input", "replace", "hello"],
        ),
        (ActionKind::InputClear, vec!["warpctrl", "input", "clear"]),
        (
            ActionKind::InputModeSet,
            vec!["warpctrl", "input", "mode", "set", "terminal"],
        ),
        (
            ActionKind::InputRun,
            vec!["warpctrl", "input", "run", "cargo check"],
        ),
        (
            ActionKind::HistoryList,
            vec!["warpctrl", "history", "list", "--limit", "5"],
        ),
        (ActionKind::ThemeList, vec!["warpctrl", "theme", "list"]),
        (ActionKind::ThemeGet, vec!["warpctrl", "theme", "get"]),
        (
            ActionKind::ThemeSet,
            vec!["warpctrl", "theme", "set", "Dracula"],
        ),
        (
            ActionKind::ThemeSystemSet,
            vec!["warpctrl", "theme", "system-set", "true"],
        ),
        (
            ActionKind::ThemeLightSet,
            vec!["warpctrl", "theme", "light-set", "Light"],
        ),
        (
            ActionKind::ThemeDarkSet,
            vec!["warpctrl", "theme", "dark-set", "Dark"],
        ),
        (
            ActionKind::AppearanceGet,
            vec!["warpctrl", "appearance", "get"],
        ),
        (
            ActionKind::AppearanceFontSizeIncrease,
            vec!["warpctrl", "appearance", "font-size-increase"],
        ),
        (
            ActionKind::AppearanceFontSizeDecrease,
            vec!["warpctrl", "appearance", "font-size-decrease"],
        ),
        (
            ActionKind::AppearanceFontSizeReset,
            vec!["warpctrl", "appearance", "font-size-reset"],
        ),
        (
            ActionKind::AppearanceZoomIncrease,
            vec!["warpctrl", "appearance", "zoom-increase"],
        ),
        (
            ActionKind::AppearanceZoomDecrease,
            vec!["warpctrl", "appearance", "zoom-decrease"],
        ),
        (
            ActionKind::AppearanceZoomReset,
            vec!["warpctrl", "appearance", "zoom-reset"],
        ),
        (ActionKind::SettingList, vec!["warpctrl", "setting", "list"]),
        (
            ActionKind::SettingGet,
            vec!["warpctrl", "setting", "get", "font_size"],
        ),
        (
            ActionKind::SettingSet,
            vec!["warpctrl", "setting", "set", "font_size", "14"],
        ),
        (
            ActionKind::SettingToggle,
            vec!["warpctrl", "setting", "toggle", "autosuggestions"],
        ),
        (
            ActionKind::KeybindingList,
            vec!["warpctrl", "keybinding", "list"],
        ),
        (
            ActionKind::KeybindingGet,
            vec!["warpctrl", "keybinding", "get", "copy"],
        ),
        (ActionKind::ActionList, vec!["warpctrl", "action", "list"]),
        (
            ActionKind::ActionInspect,
            vec!["warpctrl", "action", "inspect", "tab.create"],
        ),
        (
            ActionKind::SurfaceSettingsOpen,
            vec!["warpctrl", "surface", "settings", "open"],
        ),
        (
            ActionKind::SurfaceCommandPaletteOpen,
            vec!["warpctrl", "surface", "command-palette", "open"],
        ),
        (
            ActionKind::SurfaceCommandSearchOpen,
            vec!["warpctrl", "surface", "command-search", "open"],
        ),
        (
            ActionKind::SurfaceWarpDriveOpen,
            vec!["warpctrl", "surface", "warp-drive", "open"],
        ),
        (
            ActionKind::SurfaceWarpDriveToggle,
            vec!["warpctrl", "surface", "warp-drive", "toggle"],
        ),
        (
            ActionKind::SurfaceResourceCenterToggle,
            vec!["warpctrl", "surface", "resource-center", "toggle"],
        ),
        (
            ActionKind::SurfaceAiAssistantToggle,
            vec!["warpctrl", "surface", "ai-assistant", "toggle"],
        ),
        (
            ActionKind::SurfaceCodeReviewToggle,
            vec!["warpctrl", "surface", "code-review", "toggle"],
        ),
        (
            ActionKind::SurfaceLeftPanelToggle,
            vec!["warpctrl", "surface", "left-panel", "toggle"],
        ),
        (
            ActionKind::SurfaceRightPanelToggle,
            vec!["warpctrl", "surface", "right-panel", "toggle"],
        ),
        (
            ActionKind::SurfaceVerticalTabsToggle,
            vec!["warpctrl", "surface", "vertical-tabs", "toggle"],
        ),
        (ActionKind::FileList, vec!["warpctrl", "file", "list"]),
        (
            ActionKind::FileOpen,
            vec!["warpctrl", "file", "open", "/tmp/example.txt"],
        ),
        (
            ActionKind::ProjectActive,
            vec!["warpctrl", "project", "active"],
        ),
        (ActionKind::ProjectList, vec!["warpctrl", "project", "list"]),
        (
            ActionKind::ProjectOpen,
            vec!["warpctrl", "project", "open", "/tmp"],
        ),
        (ActionKind::DriveList, vec!["warpctrl", "drive", "list"]),
        (
            ActionKind::DriveInspect,
            vec!["warpctrl", "drive", "inspect", "object_1"],
        ),
        (
            ActionKind::DriveOpen,
            vec!["warpctrl", "drive", "open", "object_1"],
        ),
        (
            ActionKind::DriveNotebookOpen,
            vec!["warpctrl", "drive", "notebook", "open", "notebook_1"],
        ),
        (
            ActionKind::DriveEnvVarCollectionOpen,
            vec![
                "warpctrl",
                "drive",
                "env-var-collection",
                "open",
                "collection_1",
            ],
        ),
        (
            ActionKind::DriveObjectShareOpen,
            vec!["warpctrl", "drive", "object", "share", "open", "object_1"],
        ),
        (
            ActionKind::DriveObjectCreate,
            vec![
                "warpctrl", "drive", "object", "create", "--type", "workflow", "--name", "Deploy",
            ],
        ),
        (
            ActionKind::DriveObjectUpdate,
            vec![
                "warpctrl",
                "drive",
                "object",
                "update",
                "object_1",
                "--content",
                "updated",
            ],
        ),
        (
            ActionKind::DriveObjectDelete,
            vec!["warpctrl", "drive", "object", "delete", "object_1"],
        ),
        (
            ActionKind::DriveObjectInsert,
            vec!["warpctrl", "drive", "object", "insert", "object_1"],
        ),
        (
            ActionKind::DriveObjectShareToTeam,
            vec!["warpctrl", "drive", "object", "share-to-team", "object_1"],
        ),
        (
            ActionKind::DriveWorkflowRun,
            vec![
                "warpctrl",
                "drive",
                "workflow",
                "run",
                "workflow_1",
                "--arg",
                "env=prod",
            ],
        ),
    ]
}

fn parsed_action_kind(command: &ControlCommand) -> Option<ActionKind> {
    match command {
        ControlCommand::Instance(command) => match command {
            InstanceCommand::List => Some(ActionKind::InstanceList),
            InstanceCommand::Inspect(_) => Some(ActionKind::InstanceInspect),
        },
        ControlCommand::App(command) => match command {
            AppCommand::Ping(_) => Some(ActionKind::AppPing),
            AppCommand::Version(_) => Some(ActionKind::AppVersion),
            AppCommand::Active(_) => Some(ActionKind::AppActive),
            AppCommand::Focus(_) => Some(ActionKind::AppFocus),
        },
        ControlCommand::Capability(command) => match command {
            CapabilityCommand::List(_) => Some(ActionKind::CapabilityList),
            CapabilityCommand::Inspect { .. } => Some(ActionKind::CapabilityInspect),
        },
        ControlCommand::Action(command) => match command {
            ActionCatalogCommand::List(_) => Some(ActionKind::ActionList),
            ActionCatalogCommand::Inspect { .. } => Some(ActionKind::ActionInspect),
        },
        ControlCommand::Window(command) => match command {
            WindowCommand::List(_) => Some(ActionKind::WindowList),
            WindowCommand::Inspect(_) => Some(ActionKind::WindowInspect),
            WindowCommand::Create(_) => Some(ActionKind::WindowCreate),
            WindowCommand::Focus(_) => Some(ActionKind::WindowFocus),
            WindowCommand::Close(_) => Some(ActionKind::WindowClose),
        },
        ControlCommand::Tab(command) => match command {
            TabCommand::List(_) => Some(ActionKind::TabList),
            TabCommand::Inspect(_) => Some(ActionKind::TabInspect),
            TabCommand::Create(_) => Some(ActionKind::TabCreate),
            TabCommand::Activate(_) => Some(ActionKind::TabActivate),
            TabCommand::Move(_) => Some(ActionKind::TabMove),
            TabCommand::Close(_) => Some(ActionKind::TabClose),
            TabCommand::Rename(_) => Some(ActionKind::TabRename),
            TabCommand::ResetName(_) => Some(ActionKind::TabResetName),
            TabCommand::Color(command) => match command {
                TabColorCommand::Set(_) => Some(ActionKind::TabColorSet),
                TabColorCommand::Clear(_) => Some(ActionKind::TabColorClear),
            },
        },
        ControlCommand::Pane(command) => match command {
            PaneCommand::List(_) => Some(ActionKind::PaneList),
            PaneCommand::Inspect(_) => Some(ActionKind::PaneInspect),
            PaneCommand::Split(_) => Some(ActionKind::PaneSplit),
            PaneCommand::Focus(_) => Some(ActionKind::PaneFocus),
            PaneCommand::Navigate(_) => Some(ActionKind::PaneNavigate),
            PaneCommand::Resize(_) => Some(ActionKind::PaneResize),
            PaneCommand::Maximize(_) => Some(ActionKind::PaneMaximize),
            PaneCommand::Unmaximize(_) => Some(ActionKind::PaneUnmaximize),
            PaneCommand::Close(_) => Some(ActionKind::PaneClose),
            PaneCommand::Rename(_) => Some(ActionKind::PaneRename),
            PaneCommand::ResetName(_) => Some(ActionKind::PaneResetName),
        },
        ControlCommand::Session(command) => match command {
            SessionCommand::List(_) => Some(ActionKind::SessionList),
            SessionCommand::Inspect(_) => Some(ActionKind::SessionInspect),
            SessionCommand::Activate(_) => Some(ActionKind::SessionActivate),
            SessionCommand::Previous(_) => Some(ActionKind::SessionPrevious),
            SessionCommand::Next(_) => Some(ActionKind::SessionNext),
            SessionCommand::ReopenClosed(_) => Some(ActionKind::SessionReopenClosed),
        },
        ControlCommand::Block(command) => match command {
            BlockCommand::List(_) => Some(ActionKind::BlockList),
            BlockCommand::Inspect(_) => Some(ActionKind::BlockInspect),
            BlockCommand::Output(_) => Some(ActionKind::BlockOutput),
        },
        ControlCommand::Input(command) => match command {
            InputCommand::Get(_) => Some(ActionKind::InputGet),
            InputCommand::Insert(_) => Some(ActionKind::InputInsert),
            InputCommand::Replace(_) => Some(ActionKind::InputReplace),
            InputCommand::Clear(_) => Some(ActionKind::InputClear),
            InputCommand::Mode(command) => match command {
                InputModeCommand::Set(_) => Some(ActionKind::InputModeSet),
            },
            InputCommand::Run(_) => Some(ActionKind::InputRun),
        },
        ControlCommand::History(command) => match command {
            HistoryCommand::List(_) => Some(ActionKind::HistoryList),
        },
        ControlCommand::Theme(command) => match command {
            ThemeCommand::List(_) => Some(ActionKind::ThemeList),
            ThemeCommand::Get(_) => Some(ActionKind::ThemeGet),
            ThemeCommand::Set(_) => Some(ActionKind::ThemeSet),
            ThemeCommand::SystemSet(_) => Some(ActionKind::ThemeSystemSet),
            ThemeCommand::LightSet(_) => Some(ActionKind::ThemeLightSet),
            ThemeCommand::DarkSet(_) => Some(ActionKind::ThemeDarkSet),
        },
        ControlCommand::Appearance(command) => match command {
            AppearanceCommand::Get(_) => Some(ActionKind::AppearanceGet),
            AppearanceCommand::FontSizeIncrease(_) => Some(ActionKind::AppearanceFontSizeIncrease),
            AppearanceCommand::FontSizeDecrease(_) => Some(ActionKind::AppearanceFontSizeDecrease),
            AppearanceCommand::FontSizeReset(_) => Some(ActionKind::AppearanceFontSizeReset),
            AppearanceCommand::ZoomIncrease(_) => Some(ActionKind::AppearanceZoomIncrease),
            AppearanceCommand::ZoomDecrease(_) => Some(ActionKind::AppearanceZoomDecrease),
            AppearanceCommand::ZoomReset(_) => Some(ActionKind::AppearanceZoomReset),
        },
        ControlCommand::Setting(command) => match command {
            SettingCommand::List(_) => Some(ActionKind::SettingList),
            SettingCommand::Get(_) => Some(ActionKind::SettingGet),
            SettingCommand::Set(_) => Some(ActionKind::SettingSet),
            SettingCommand::Toggle(_) => Some(ActionKind::SettingToggle),
        },
        ControlCommand::Keybinding(command) => match command {
            KeybindingCommand::List(_) => Some(ActionKind::KeybindingList),
            KeybindingCommand::Get(_) => Some(ActionKind::KeybindingGet),
        },
        ControlCommand::File(command) => match command {
            FileCommand::List(_) => Some(ActionKind::FileList),
            FileCommand::Open(_) => Some(ActionKind::FileOpen),
        },
        ControlCommand::Project(command) => match command {
            ProjectCommand::Active(_) => Some(ActionKind::ProjectActive),
            ProjectCommand::List(_) => Some(ActionKind::ProjectList),
            ProjectCommand::Open(_) => Some(ActionKind::ProjectOpen),
        },
        ControlCommand::Drive(command) => match command {
            DriveCommand::List(_) => Some(ActionKind::DriveList),
            DriveCommand::Inspect(_) => Some(ActionKind::DriveInspect),
            DriveCommand::Open(_) => Some(ActionKind::DriveOpen),
            DriveCommand::Notebook(command) => match command {
                DriveObjectOpenCommand::Open(_) => Some(ActionKind::DriveNotebookOpen),
            },
            DriveCommand::EnvVarCollection(command) => match command {
                DriveObjectOpenCommand::Open(_) => Some(ActionKind::DriveEnvVarCollectionOpen),
            },
            DriveCommand::Object(command) => match command {
                DriveObjectCommand::Share(command) => match command {
                    DriveObjectShareCommand::Open(_) => Some(ActionKind::DriveObjectShareOpen),
                },
                DriveObjectCommand::Create(_) => Some(ActionKind::DriveObjectCreate),
                DriveObjectCommand::Update(_) => Some(ActionKind::DriveObjectUpdate),
                DriveObjectCommand::Delete(_) => Some(ActionKind::DriveObjectDelete),
                DriveObjectCommand::Insert(_) => Some(ActionKind::DriveObjectInsert),
                DriveObjectCommand::ShareToTeam(_) => Some(ActionKind::DriveObjectShareToTeam),
            },
            DriveCommand::Workflow(command) => match command {
                DriveWorkflowCommand::Run(_) => Some(ActionKind::DriveWorkflowRun),
            },
        },
        ControlCommand::Surface(command) => match command {
            SurfaceCommand::Settings(command) => match command {
                SurfaceSettingsCommand::Open(_) => Some(ActionKind::SurfaceSettingsOpen),
            },
            SurfaceCommand::CommandPalette(command) => match command {
                SurfaceQueryCommand::Open(_) => Some(ActionKind::SurfaceCommandPaletteOpen),
            },
            SurfaceCommand::CommandSearch(command) => match command {
                SurfaceQueryCommand::Open(_) => Some(ActionKind::SurfaceCommandSearchOpen),
            },
            SurfaceCommand::WarpDrive(command) => match command {
                SurfaceOpenToggleCommand::Open(_) => Some(ActionKind::SurfaceWarpDriveOpen),
                SurfaceOpenToggleCommand::Toggle(_) => Some(ActionKind::SurfaceWarpDriveToggle),
            },
            SurfaceCommand::ResourceCenter(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceResourceCenterToggle),
            },
            SurfaceCommand::AiAssistant(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceAiAssistantToggle),
            },
            SurfaceCommand::CodeReview(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceCodeReviewToggle),
            },
            SurfaceCommand::LeftPanel(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceLeftPanelToggle),
            },
            SurfaceCommand::RightPanel(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceRightPanelToggle),
            },
            SurfaceCommand::VerticalTabs(command) => match command {
                SurfaceToggleCommand::Toggle(_) => Some(ActionKind::SurfaceVerticalTabsToggle),
            },
        },
        ControlCommand::Completions { .. } => None,
    }
}
