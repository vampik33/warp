use std::io::Write as _;
use std::process::ExitCode;
use std::str::FromStr;

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use clap_complete::aot::{Shell, generate};
use local_control::protocol::{
    Action, ActionKind, ActionMetadata, AppFocusParams, AppSurfaceParams, AppearanceFontSizeParams,
    AppearanceSetParams, AppearanceZoomParams, BlockSelector, BlockTarget, ControlError,
    DriveCreateParams, DriveDeleteParams, DriveInsertParams, DriveObjectSelector, DriveRunParams,
    DriveTarget, DriveUpdateParams, ErrorCode, FileDeleteParams, FileOpenParams, FileSelector,
    FileTarget, FileWriteParams, HorizontalDirection, InputClearParams, InputInsertParams,
    InputMode, InputModeSetParams, InputReplaceParams, InputRunParams, PaneCloseParams,
    PaneDirection, PaneFocusParams, PaneMaximizeParams, PaneNavigateParams, PaneResizeParams,
    PaneSelector, PaneSplitParams, PaneTarget, RequestEnvelope, SessionSelector, SessionTarget,
    SettingSetParams, SettingToggleParams, SizeAdjustment, TabActivateParams, TabActivationTarget,
    TabCloseParams, TabCloseScope, TabMoveParams, TabRenameParams, TabSelector, TabTarget,
    TargetSelector, ThemeSetParams, WindowCloseParams, WindowCreateParams, WindowFocusParams,
    WindowSelector, WindowTarget,
};
use local_control::selection::{InstanceSelector, select_instance};
use serde::Serialize;

use crate::agent::OutputFormat;

#[derive(Debug, Parser)]
#[command(
    name = "warpctrl",
    display_name = "warpctrl",
    about = "Control a running local Warp app instance"
)]
pub struct ControlArgs {
    /// Set the output format.
    #[arg(
        long = "output-format",
        global = true,
        value_enum,
        default_value_t = OutputFormat::Pretty,
        env = "WARP_OUTPUT_FORMAT"
    )]
    pub output_format: OutputFormat,

    #[command(subcommand)]
    pub command: ControlCommand,
}

#[derive(Debug, Clone, Args, Default)]
struct WindowTargetArgs {
    /// Target a window with active, id:<id>, index:<n>, or title:<title>.
    #[arg(
        long = "window",
        value_name = "SELECTOR",
        conflicts_with_all = ["window_id", "window_index", "window_title"]
    )]
    window: Option<WindowTargetArg>,

    /// Target a window by opaque id.
    #[arg(
        long = "window-id",
        value_name = "ID",
        conflicts_with_all = ["window", "window_index", "window_title"]
    )]
    window_id: Option<String>,

    /// Target a window by zero-based index.
    #[arg(
        long = "window-index",
        value_name = "INDEX",
        conflicts_with_all = ["window", "window_id", "window_title"]
    )]
    window_index: Option<u32>,

    /// Target a window by exact title.
    #[arg(
        long = "window-title",
        value_name = "TITLE",
        conflicts_with_all = ["window", "window_id", "window_index"]
    )]
    window_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowTargetArg {
    Active,
    Id(String),
    Index(u32),
    Title(String),
}

impl WindowTargetArgs {
    fn to_target(&self) -> Option<WindowTarget> {
        if let Some(target) = &self.window {
            return Some(match target {
                WindowTargetArg::Active => WindowTarget::Active,
                WindowTargetArg::Id(id) => WindowTarget::Id {
                    id: WindowSelector(id.clone()),
                },
                WindowTargetArg::Index(index) => WindowTarget::Index { index: *index },
                WindowTargetArg::Title(title) => WindowTarget::Title {
                    title: title.clone(),
                },
            });
        }
        if let Some(id) = &self.window_id {
            return Some(WindowTarget::Id {
                id: WindowSelector(id.clone()),
            });
        }
        if let Some(index) = self.window_index {
            return Some(WindowTarget::Index { index });
        }
        self.window_title.as_ref().map(|title| WindowTarget::Title {
            title: title.clone(),
        })
    }

    fn has_target(&self) -> bool {
        self.window.is_some()
            || self.window_id.is_some()
            || self.window_index.is_some()
            || self.window_title.is_some()
    }
}

impl FromStr for WindowTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "active" {
            return Ok(Self::Active);
        }
        let (kind, selector) = parse_selector_parts(value, "window")?;
        match kind {
            "id" => Ok(Self::Id(non_empty_selector_value(selector, "window id")?)),
            "index" => Ok(Self::Index(parse_selector_index(selector, "window index")?)),
            "title" => Ok(Self::Title(non_empty_selector_value(
                selector,
                "window title",
            )?)),
            _ => Err(format!(
                "invalid window selector {value}; expected active, id:<id>, index:<n>, or title:<title>"
            )),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct TabTargetArgs {
    /// Target a tab with active, id:<id>, index:<n>, or title:<title>.
    #[arg(
        long = "tab",
        value_name = "SELECTOR",
        conflicts_with_all = ["tab_id", "tab_index", "tab_title"]
    )]
    tab: Option<TabTargetArg>,

    /// Target a tab by opaque id.
    #[arg(
        long = "tab-id",
        value_name = "ID",
        conflicts_with_all = ["tab", "tab_index", "tab_title"]
    )]
    tab_id: Option<String>,

    /// Target a tab by zero-based index.
    #[arg(
        long = "tab-index",
        value_name = "INDEX",
        conflicts_with_all = ["tab", "tab_id", "tab_title"]
    )]
    tab_index: Option<u32>,

    /// Target a tab by exact title.
    #[arg(
        long = "tab-title",
        value_name = "TITLE",
        conflicts_with_all = ["tab", "tab_id", "tab_index"]
    )]
    tab_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TabTargetArg {
    Active,
    Id(String),
    Index(u32),
    Title(String),
}

impl TabTargetArgs {
    fn to_target(&self) -> Option<TabTarget> {
        if let Some(target) = &self.tab {
            return Some(match target {
                TabTargetArg::Active => TabTarget::Active,
                TabTargetArg::Id(id) => TabTarget::Id {
                    id: TabSelector(id.clone()),
                },
                TabTargetArg::Index(index) => TabTarget::Index { index: *index },
                TabTargetArg::Title(title) => TabTarget::Title {
                    title: title.clone(),
                },
            });
        }
        if let Some(id) = &self.tab_id {
            return Some(TabTarget::Id {
                id: TabSelector(id.clone()),
            });
        }
        if let Some(index) = self.tab_index {
            return Some(TabTarget::Index { index });
        }
        self.tab_title.as_ref().map(|title| TabTarget::Title {
            title: title.clone(),
        })
    }

    fn has_target(&self) -> bool {
        self.tab.is_some()
            || self.tab_id.is_some()
            || self.tab_index.is_some()
            || self.tab_title.is_some()
    }
}

impl FromStr for TabTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "active" {
            return Ok(Self::Active);
        }
        let (kind, selector) = parse_selector_parts(value, "tab")?;
        match kind {
            "id" => Ok(Self::Id(non_empty_selector_value(selector, "tab id")?)),
            "index" => Ok(Self::Index(parse_selector_index(selector, "tab index")?)),
            "title" => Ok(Self::Title(non_empty_selector_value(
                selector,
                "tab title",
            )?)),
            _ => Err(format!(
                "invalid tab selector {value}; expected active, id:<id>, index:<n>, or title:<title>"
            )),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct PaneTargetArgs {
    /// Target a pane with active, id:<id>, or index:<n>.
    #[arg(
        long = "pane",
        value_name = "SELECTOR",
        conflicts_with_all = ["pane_id", "pane_index"]
    )]
    pane: Option<PaneTargetArg>,

    /// Target a pane by opaque id.
    #[arg(
        long = "pane-id",
        value_name = "ID",
        conflicts_with_all = ["pane", "pane_index"]
    )]
    pane_id: Option<String>,

    /// Target a pane by zero-based index.
    #[arg(
        long = "pane-index",
        value_name = "INDEX",
        conflicts_with_all = ["pane", "pane_id"]
    )]
    pane_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PaneTargetArg {
    Active,
    Id(String),
    Index(u32),
}

impl PaneTargetArgs {
    fn to_target(&self) -> Option<PaneTarget> {
        if let Some(target) = &self.pane {
            return Some(match target {
                PaneTargetArg::Active => PaneTarget::Active,
                PaneTargetArg::Id(id) => PaneTarget::Id {
                    id: PaneSelector(id.clone()),
                },
                PaneTargetArg::Index(index) => PaneTarget::Index { index: *index },
            });
        }
        if let Some(id) = &self.pane_id {
            return Some(PaneTarget::Id {
                id: PaneSelector(id.clone()),
            });
        }
        self.pane_index.map(|index| PaneTarget::Index { index })
    }

    fn has_target(&self) -> bool {
        self.pane.is_some() || self.pane_id.is_some() || self.pane_index.is_some()
    }
}

impl FromStr for PaneTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "active" {
            return Ok(Self::Active);
        }
        let (kind, selector) = parse_selector_parts(value, "pane")?;
        match kind {
            "id" => Ok(Self::Id(non_empty_selector_value(selector, "pane id")?)),
            "index" => Ok(Self::Index(parse_selector_index(selector, "pane index")?)),
            _ => Err(format!(
                "invalid pane selector {value}; expected active, id:<id>, or index:<n>"
            )),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct SessionTargetArgs {
    /// Target a session with active or id:<id>.
    #[arg(
        long = "session",
        value_name = "SELECTOR",
        conflicts_with = "session_id"
    )]
    session: Option<SessionTargetArg>,

    /// Target a session by opaque id.
    #[arg(long = "session-id", value_name = "ID", conflicts_with = "session")]
    session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionTargetArg {
    Active,
    Id(String),
}

impl SessionTargetArgs {
    fn to_target(&self) -> Option<SessionTarget> {
        if let Some(target) = &self.session {
            return Some(match target {
                SessionTargetArg::Active => SessionTarget::Active,
                SessionTargetArg::Id(id) => SessionTarget::Id {
                    id: SessionSelector(id.clone()),
                },
            });
        }
        self.session_id.as_ref().map(|id| SessionTarget::Id {
            id: SessionSelector(id.clone()),
        })
    }

    fn has_target(&self) -> bool {
        self.session.is_some() || self.session_id.is_some()
    }
}

impl FromStr for SessionTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "active" {
            return Ok(Self::Active);
        }
        let (kind, selector) = parse_selector_parts(value, "session")?;
        match kind {
            "id" => Ok(Self::Id(non_empty_selector_value(selector, "session id")?)),
            _ => Err(format!(
                "invalid session selector {value}; expected active or id:<id>"
            )),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct BlockTargetArgs {
    /// Target a block with active, id:<id>, or index:<n>.
    #[arg(
        long = "block",
        value_name = "SELECTOR",
        conflicts_with_all = ["block_id_selector", "block_index"]
    )]
    block: Option<BlockTargetArg>,

    /// Target a block by opaque id.
    #[arg(
        long = "block-id",
        value_name = "ID",
        id = "block_id_selector",
        conflicts_with_all = ["block", "block_index"]
    )]
    block_id: Option<String>,

    /// Target a block by zero-based index.
    #[arg(
        long = "block-index",
        value_name = "INDEX",
        conflicts_with_all = ["block", "block_id_selector"]
    )]
    block_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockTargetArg {
    Active,
    Id(String),
    Index(u32),
}

impl BlockTargetArgs {
    fn to_target(&self) -> Option<BlockTarget> {
        if let Some(target) = &self.block {
            return Some(match target {
                BlockTargetArg::Active => BlockTarget::Active,
                BlockTargetArg::Id(id) => BlockTarget::Id {
                    id: BlockSelector(id.clone()),
                },
                BlockTargetArg::Index(index) => BlockTarget::Index { index: *index },
            });
        }
        if let Some(id) = &self.block_id {
            return Some(BlockTarget::Id {
                id: BlockSelector(id.clone()),
            });
        }
        self.block_index.map(|index| BlockTarget::Index { index })
    }

    fn has_target(&self) -> bool {
        self.block.is_some() || self.block_id.is_some() || self.block_index.is_some()
    }
}

impl FromStr for BlockTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "active" {
            return Ok(Self::Active);
        }
        let (kind, selector) = parse_selector_parts(value, "block")?;
        match kind {
            "id" => Ok(Self::Id(non_empty_selector_value(selector, "block id")?)),
            "index" => Ok(Self::Index(parse_selector_index(selector, "block index")?)),
            _ => Err(format!(
                "invalid block selector {value}; expected active, id:<id>, or index:<n>"
            )),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct FileTargetArgs {
    /// Target a file with path:<path> or id:<id>.
    #[arg(
        long = "file",
        value_name = "SELECTOR",
        conflicts_with_all = ["file_path", "file_id"]
    )]
    file: Option<FileTargetArg>,

    /// Target a file by path.
    #[arg(
        long = "file-path",
        value_name = "PATH",
        conflicts_with_all = ["file", "file_id"]
    )]
    file_path: Option<String>,

    /// Target a file by opaque id.
    #[arg(
        long = "file-id",
        value_name = "ID",
        conflicts_with_all = ["file", "file_path"]
    )]
    file_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileTargetArg {
    Path(String),
    Id(String),
}

impl FileTargetArgs {
    fn to_target(&self) -> Option<FileTarget> {
        if let Some(target) = &self.file {
            return Some(match target {
                FileTargetArg::Path(path) => FileTarget::Path { path: path.clone() },
                FileTargetArg::Id(id) => FileTarget::Id {
                    id: FileSelector(id.clone()),
                },
            });
        }
        if let Some(path) = &self.file_path {
            return Some(FileTarget::Path { path: path.clone() });
        }
        self.file_id.as_ref().map(|id| FileTarget::Id {
            id: FileSelector(id.clone()),
        })
    }

    fn has_target(&self) -> bool {
        self.file.is_some() || self.file_path.is_some() || self.file_id.is_some()
    }
}

impl FromStr for FileTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (kind, selector) = parse_selector_parts(value, "file")?;
        match kind {
            "path" => Ok(Self::Path(non_empty_selector_value(selector, "file path")?)),
            "id" => Ok(Self::Id(non_empty_selector_value(selector, "file id")?)),
            _ => Err(format!(
                "invalid file selector {value}; expected path:<path> or id:<id>"
            )),
        }
    }
}

#[derive(Debug, Clone, Args, Default)]
struct DriveTargetArgs {
    /// Target a Drive object with id:<type>:<id> or name:<type>:<name>.
    #[arg(
        long = "drive",
        value_name = "SELECTOR",
        conflicts_with_all = ["drive_id", "drive_name"]
    )]
    drive: Option<DriveTargetArg>,

    /// Target a Drive object by type and opaque id, such as workflow:abc123.
    #[arg(
        long = "drive-id",
        value_name = "TYPE:ID",
        conflicts_with_all = ["drive", "drive_name"]
    )]
    drive_id: Option<DriveObjectValueArg>,

    /// Target a Drive object by type and exact name, such as workflow:Deploy.
    #[arg(
        long = "drive-name",
        value_name = "TYPE:NAME",
        conflicts_with_all = ["drive", "drive_id"]
    )]
    drive_name: Option<DriveObjectValueArg>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DriveTargetArg {
    Id {
        object_type: local_control::DriveObjectType,
        id: String,
    },
    Name {
        object_type: local_control::DriveObjectType,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DriveObjectValueArg {
    object_type: local_control::DriveObjectType,
    value: String,
}

impl DriveTargetArgs {
    fn to_target(&self) -> Option<DriveTarget> {
        if let Some(target) = &self.drive {
            return Some(match target {
                DriveTargetArg::Id { object_type, id } => DriveTarget::Id {
                    object_type: *object_type,
                    id: DriveObjectSelector(id.clone()),
                },
                DriveTargetArg::Name { object_type, name } => DriveTarget::Name {
                    object_type: *object_type,
                    name: name.clone(),
                },
            });
        }
        if let Some(value) = &self.drive_id {
            return Some(DriveTarget::Id {
                object_type: value.object_type,
                id: DriveObjectSelector(value.value.clone()),
            });
        }
        self.drive_name.as_ref().map(|value| DriveTarget::Name {
            object_type: value.object_type,
            name: value.value.clone(),
        })
    }

    fn has_target(&self) -> bool {
        self.drive.is_some() || self.drive_id.is_some() || self.drive_name.is_some()
    }
}

impl FromStr for DriveTargetArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (kind, selector) = parse_selector_parts(value, "drive")?;
        let parsed = selector.parse::<DriveObjectValueArg>()?;
        match kind {
            "id" => Ok(Self::Id {
                object_type: parsed.object_type,
                id: parsed.value,
            }),
            "name" => Ok(Self::Name {
                object_type: parsed.object_type,
                name: parsed.value,
            }),
            _ => Err(format!(
                "invalid drive selector {value}; expected id:<type>:<id> or name:<type>:<name>"
            )),
        }
    }
}

impl FromStr for DriveObjectValueArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (object_type, selector) = value
            .split_once(':')
            .ok_or_else(|| format!("invalid Drive selector {value}; expected type:value"))?;
        Ok(Self {
            object_type: parse_drive_object_type(object_type)?,
            value: non_empty_selector_value(selector, "Drive selector value")?,
        })
    }
}
impl TargetArgs {
    fn instance_selector(&self) -> InstanceSelector {
        if let Some(instance_id) = &self.instance {
            return InstanceSelector::Id(local_control::discovery::InstanceId(instance_id.clone()));
        }
        if let Some(pid) = self.pid {
            return InstanceSelector::Pid(pid);
        }
        InstanceSelector::Active
    }

    fn target_selector(&self) -> TargetSelector {
        TargetSelector {
            window: self.window.to_target(),
            tab: self.tab.to_target(),
            pane: self.pane.to_target(),
            session: self.session.to_target(),
            block: self.block.to_target(),
            file: self.file.to_target(),
            drive: self.drive.to_target(),
        }
    }

    fn has_app_target(&self) -> bool {
        self.window.has_target()
            || self.tab.has_target()
            || self.pane.has_target()
            || self.session.has_target()
            || self.block.has_target()
            || self.file.has_target()
            || self.drive.has_target()
    }
}

fn parse_selector_parts<'a>(value: &'a str, family: &str) -> Result<(&'a str, &'a str), String> {
    value
        .split_once(':')
        .ok_or_else(|| format!("invalid {family} selector {value}; expected kind:value"))
}

fn non_empty_selector_value(value: &str, label: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }
    Ok(value.to_owned())
}

fn parse_selector_index(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("{label} must be a non-negative integer"))
}

fn parse_drive_object_type(value: &str) -> Result<local_control::DriveObjectType, String> {
    match value {
        "workflow" => Ok(local_control::DriveObjectType::Workflow),
        "notebook" => Ok(local_control::DriveObjectType::Notebook),
        "environment" => Ok(local_control::DriveObjectType::Environment),
        "prompt" => Ok(local_control::DriveObjectType::Prompt),
        _ => Err(format!(
            "invalid Drive object type {value}; expected workflow, notebook, environment, or prompt"
        )),
    }
}

impl ControlArgs {
    pub fn from_env() -> Self {
        let matches = Self::clap_command().get_matches();
        Self::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
    }

    pub fn clap_command() -> clap::Command {
        let bin_name = crate::binary_name().unwrap_or_else(|| "warpctrl".to_owned());
        <Self as CommandFactory>::command()
            .version(crate::version_string())
            .bin_name(bin_name.clone())
            .after_help(color_print::cformat!(
                r#"<bold><underline>Examples:</underline></bold>

  <dim>$</dim> <bold>{bin_name} instance list</bold>

  <dim>$</dim> <bold>{bin_name} tab create</bold>

<bold><underline>Learn more:</underline></bold>
* Use <bold>{bin_name} help</bold> to learn more about each command
"#
            ))
    }
}

fn run_app_surface_command(
    args: AppSurfaceArgs,
    action: ActionKind,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    reject_app_target(&args.target, action)?;
    run_action_with_params(
        args.target,
        action,
        AppSurfaceParams {
            query: args.query,
            page: args.page,
        },
        output_format,
    )
}

fn run_tab_activate_relative(
    args: TargetArgs,
    relative: TabActivationTarget,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    run_action_with_params(
        args,
        ActionKind::TabActivate,
        TabActivateParams {
            relative: Some(relative),
        },
        output_format,
    )
}

fn parse_json_value_or_string(value: String) -> serde_json::Value {
    match serde_json::from_str(&value) {
        Ok(value) => value,
        Err(_) => serde_json::Value::String(value),
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum ControlCommand {
    /// Inspect local Warp app instances.
    #[command(subcommand)]
    Instance(InstanceCommand),
    /// Inspect a selected local Warp app.
    #[command(subcommand)]
    App(AppCommand),
    /// Inspect the local-control action catalog.
    #[command(subcommand)]
    Action(ActionCommand),
    /// Inspect local Warp windows.
    #[command(subcommand)]
    Window(WindowCommand),
    /// Control local Warp tabs.
    #[command(subcommand)]
    Tab(TabCommand),
    /// Inspect local Warp panes.
    #[command(subcommand)]
    Pane(PaneCommand),
    /// Inspect local Warp sessions.
    #[command(subcommand)]
    Session(SessionCommand),
    /// Inspect terminal blocks.
    #[command(subcommand)]
    Block(BlockCommand),
    /// Inspect terminal input state.
    #[command(subcommand)]
    Input(InputCommand),
    /// Inspect terminal command history.
    #[command(subcommand)]
    History(HistoryCommand),
    /// Inspect Warp themes.
    #[command(subcommand)]
    Theme(ThemeCommand),
    /// Inspect appearance state.
    #[command(subcommand)]
    Appearance(AppearanceCommand),
    /// Inspect allowlisted settings.
    #[command(subcommand)]
    Setting(SettingCommand),
    /// Inspect files currently surfaced in Warp.
    #[command(subcommand)]
    File(FileCommand),
    /// Inspect projects currently known to Warp.
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Inspect Warp Drive objects.
    #[command(subcommand)]
    Drive(DriveCommand),

    /// Generate shell completions for your shell to stdout.
    ///
    /// For bash, add the following to ~/.bashrc:
    ///     source <(path/to/warpctrl completions bash)
    ///
    /// For zsh, add the following to ~/.zshrc:
    ///     source <(path/to/warpctrl completions zsh)
    ///
    /// For fish, add the following to ~/.config/fish/config.fish:
    ///     path/to/warpctrl completions fish | source
    ///
    /// For Powershell, add the following to $PROFILE:
    ///     path\to\warpctrl completions powershell | Out-String | Invoke-Expression
    ///
    /// If no shell is provided, this defaults to the shell that Warp was run from.
    #[command(verbatim_doc_comment)]
    Completions {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: Option<Shell>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum InstanceCommand {
    /// List locally discoverable Warp instances.
    List,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AppCommand {
    /// Check that the selected local Warp app responds.
    Ping(TargetArgs),
    /// Print protocol and app version metadata for the selected local Warp app.
    Version(TargetArgs),
    /// Print the active window/tab/pane/session chain.
    Active(TargetArgs),
    /// Print app and protocol metadata.
    Inspect(TargetArgs),
    /// Focus the selected Warp app instance.
    Focus(TargetArgs),
    /// Open the Settings surface.
    SettingsOpen(AppSurfaceArgs),
    /// Open the Command Palette.
    CommandPaletteOpen(AppSurfaceArgs),
    /// Open command search.
    CommandSearchOpen(AppSurfaceArgs),
    /// Open Warp Drive.
    WarpDriveOpen(AppSurfaceArgs),
    /// Toggle Warp Drive.
    WarpDriveToggle(AppSurfaceArgs),
    /// Toggle the resource center.
    ResourceCenterToggle(AppSurfaceArgs),
    /// Toggle the AI assistant surface.
    AiAssistantToggle(AppSurfaceArgs),
    /// Toggle the code review surface.
    CodeReviewToggle(AppSurfaceArgs),
    /// Toggle the vertical tabs panel.
    VerticalTabsToggle(AppSurfaceArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ActionCommand {
    /// List allowlisted local-control actions.
    List(TargetArgs),
    /// Inspect one allowlisted local-control action.
    Get(ActionGetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum WindowCommand {
    /// List windows in the selected local Warp app.
    List(TargetArgs),
    /// Create a new Warp window.
    Create(WindowCreateArgs),
    /// Focus a Warp window.
    Focus(TargetArgs),
    /// Close a Warp window.
    Close(WindowCloseArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum TabCommand {
    /// List tabs in the selected local Warp app.
    List(TargetArgs),
    /// Create a new terminal tab in the active window.
    Create(TargetArgs),
    /// Activate a target tab.
    Activate(TargetArgs),
    /// Activate the previous tab.
    Previous(TargetArgs),
    /// Activate the next tab.
    Next(TargetArgs),
    /// Activate the last tab.
    Last(TargetArgs),
    /// Move a target tab left or right.
    Move(TabMoveArgs),
    /// Rename or reset a target tab title.
    Rename(TabRenameArgs),
    /// Close a target tab or tab group.
    Close(TabCloseArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum PaneCommand {
    /// List panes in the selected local Warp app.
    List(TargetArgs),
    /// Split a pane.
    Split(PaneSplitArgs),
    /// Focus a pane.
    Focus(TargetArgs),
    /// Navigate pane focus.
    Navigate(PaneNavigateArgs),
    /// Close a pane.
    Close(PaneCloseArgs),
    /// Toggle or set pane maximization.
    Maximize(PaneMaximizeArgs),
    /// Resize a pane divider.
    Resize(PaneResizeArgs),
    /// Switch to the previous session in a pane.
    PreviousSession(TargetArgs),
    /// Switch to the next session in a pane.
    NextSession(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommand {
    /// List sessions in the selected local Warp app.
    List(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum BlockCommand {
    /// List terminal blocks.
    List(LimitTargetArgs),
    /// Read one terminal block.
    Get(BlockGetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum InputCommand {
    /// Read the current input buffer.
    Get(TargetArgs),
    /// Insert text into the active input buffer.
    Insert(InputInsertArgs),
    /// Replace the active input buffer.
    Replace(InputTextArgs),
    /// Clear the active input buffer.
    Clear(TargetArgs),
    /// Set the active input mode.
    Mode(InputModeArgs),
    /// Run a command in the target session.
    Run(InputRunArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum HistoryCommand {
    /// List command history entries.
    List(LimitTargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ThemeCommand {
    /// List available themes.
    List(TargetArgs),
    /// Set the current theme.
    Set(ThemeSetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum AppearanceCommand {
    /// Read appearance state.
    Get(TargetArgs),
    /// Set theme-following appearance state.
    Set(AppearanceSetArgs),
    /// Adjust font size.
    FontSize(AppearanceAdjustArgs),
    /// Adjust UI zoom.
    Zoom(AppearanceAdjustArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SettingCommand {
    /// List allowlisted settings.
    List(TargetArgs),
    /// Read one allowlisted setting.
    Get(SettingGetArgs),
    /// Set one allowlisted setting.
    Set(SettingSetArgsCli),
    /// Toggle one allowlisted boolean setting.
    Toggle(SettingToggleArgsCli),
}

#[derive(Debug, Clone, Subcommand)]
pub enum FileCommand {
    /// List files currently surfaced in Warp.
    List(TargetArgs),
    /// Open a path in Warp.
    Open(FileOpenArgs),
    /// Write a file through the local-control protocol.
    Write(FileWriteArgs),
    /// Delete a file through the local-control protocol.
    Delete(FileDeleteArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectCommand {
    /// Print the active project for the selected local Warp app.
    Active(TargetArgs),
    /// List projects currently known to Warp.
    List(TargetArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum DriveCommand {
    /// List Warp Drive objects.
    List(DriveListArgs),
    /// Read one Warp Drive object.
    Get(DriveGetArgs),
    /// Create a Warp Drive object.
    Create(DriveCreateArgs),
    /// Update a Warp Drive object.
    Update(DriveUpdateArgs),
    /// Delete a Warp Drive object.
    Delete(DriveObjectMutationArgs),
    /// Run a Warp Drive workflow.
    Run(DriveObjectMutationArgs),
    /// Insert a Warp Drive object into the active input.
    Insert(DriveObjectMutationArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct TargetArgs {
    /// Target a specific local Warp instance id from `warp instance list`.
    #[arg(long = "instance")]
    pub instance: Option<String>,

    /// Target a specific local Warp process id.
    #[arg(long = "pid", conflicts_with = "instance")]
    pub pid: Option<u32>,

    #[command(flatten)]
    window: WindowTargetArgs,

    #[command(flatten)]
    tab: TabTargetArgs,

    #[command(flatten)]
    pane: PaneTargetArgs,

    #[command(flatten)]
    session: SessionTargetArgs,

    #[command(flatten)]
    block: BlockTargetArgs,

    #[command(flatten)]
    file: FileTargetArgs,

    #[command(flatten)]
    drive: DriveTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub struct AppSurfaceArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "query")]
    pub query: Option<String>,

    #[arg(long = "page")]
    pub page: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct WindowCreateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "profile")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct WindowCloseArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "force")]
    pub force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TabMoveArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: HorizontalDirectionArg,
}

#[derive(Debug, Clone, Args)]
pub struct TabRenameArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub title: Option<String>,

    #[arg(long = "reset")]
    pub reset: bool,
}

#[derive(Debug, Clone, Args)]
pub struct TabCloseArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "scope", value_enum, default_value_t = TabCloseScopeArg::Target)]
    pub scope: TabCloseScopeArg,

    #[arg(long = "force")]
    pub force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct PaneSplitArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: PaneDirectionArg,

    #[arg(long = "profile")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct PaneNavigateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: PaneDirectionArg,
}

#[derive(Debug, Clone, Args)]
pub struct PaneCloseArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "force")]
    pub force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct PaneMaximizeArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "enabled")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Args)]
pub struct PaneResizeArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "direction", value_enum)]
    pub direction: PaneDirectionArg,

    #[arg(long = "amount")]
    pub amount: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct InputInsertArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub text: String,

    #[arg(long = "replace")]
    pub replace: bool,
}

#[derive(Debug, Clone, Args)]
pub struct InputTextArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub text: String,
}

#[derive(Debug, Clone, Args)]
pub struct InputModeArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(value_enum)]
    pub mode: InputModeArg,
}

#[derive(Debug, Clone, Args)]
pub struct InputRunArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub command: String,
}

#[derive(Debug, Clone, Args)]
pub struct ThemeSetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub name: String,
}

#[derive(Debug, Clone, Args)]
pub struct AppearanceSetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(long = "theme")]
    pub theme: Option<String>,

    #[arg(long = "follow-system-theme")]
    pub follow_system_theme: Option<bool>,

    #[arg(long = "light-theme")]
    pub light_theme: Option<String>,

    #[arg(long = "dark-theme")]
    pub dark_theme: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct AppearanceAdjustArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    #[arg(value_enum)]
    pub adjustment: SizeAdjustmentArg,

    #[arg(long = "value")]
    pub value: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct SettingSetArgsCli {
    #[command(flatten)]
    pub target: TargetArgs,

    pub key: String,

    pub value: String,
}

#[derive(Debug, Clone, Args)]
pub struct SettingToggleArgsCli {
    #[command(flatten)]
    pub target: TargetArgs,

    pub key: String,
}

#[derive(Debug, Clone, Args)]
pub struct FileOpenArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub path: String,

    #[arg(long = "line")]
    pub line: Option<u32>,

    #[arg(long = "new-window")]
    pub new_window: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FileWriteArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub path: String,

    pub contents: String,

    #[arg(long = "create")]
    pub create: bool,
}

#[derive(Debug, Clone, Args)]
pub struct FileDeleteArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    pub path: String,

    #[arg(long = "recursive")]
    pub recursive: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ActionGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Action name, such as tab.create or window.list.
    pub action: String,
}

#[derive(Debug, Clone, Args)]
pub struct LimitTargetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Maximum number of items to return.
    #[arg(long = "limit")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Args)]
pub struct BlockGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Opaque block id returned by block list.
    pub block_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct SettingGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Allowlisted setting key.
    pub key: String,
}

#[derive(Debug, Clone, Args)]
pub struct DriveListArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Restrict results to one Drive object type.
    #[arg(long = "type")]
    pub object_type: Option<DriveObjectTypeArg>,
}

#[derive(Debug, Clone, Args)]
pub struct DriveGetArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Warp Drive object type.
    #[arg(long = "type")]
    pub object_type: DriveObjectTypeArg,

    /// Opaque Warp Drive object id.
    pub id: String,
}
#[derive(Debug, Clone, Args)]
pub struct DriveCreateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Warp Drive object type.
    #[arg(long = "type")]
    pub object_type: DriveObjectTypeArg,

    /// Name for the new Drive object.
    pub name: String,

    /// Object content, parsed as JSON when possible and otherwise treated as a string.
    pub content: String,
}

#[derive(Debug, Clone, Args)]
pub struct DriveUpdateArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Warp Drive object type.
    #[arg(long = "type")]
    pub object_type: DriveObjectTypeArg,

    /// Opaque Warp Drive object id.
    pub id: String,

    /// Object content, parsed as JSON when possible and otherwise treated as a string.
    pub content: String,
}

#[derive(Debug, Clone, Args)]
pub struct DriveObjectMutationArgs {
    #[command(flatten)]
    pub target: TargetArgs,

    /// Warp Drive object type.
    #[arg(long = "type")]
    pub object_type: DriveObjectTypeArg,

    /// Opaque Warp Drive object id.
    pub id: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DriveObjectTypeArg {
    Workflow,
    Notebook,
    Environment,
    Prompt,
}

impl From<DriveObjectTypeArg> for local_control::DriveObjectType {
    fn from(value: DriveObjectTypeArg) -> Self {
        match value {
            DriveObjectTypeArg::Workflow => Self::Workflow,
            DriveObjectTypeArg::Notebook => Self::Notebook,
            DriveObjectTypeArg::Environment => Self::Environment,
            DriveObjectTypeArg::Prompt => Self::Prompt,
        }
    }
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HorizontalDirectionArg {
    Left,
    Right,
}

impl From<HorizontalDirectionArg> for HorizontalDirection {
    fn from(value: HorizontalDirectionArg) -> Self {
        match value {
            HorizontalDirectionArg::Left => Self::Left,
            HorizontalDirectionArg::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TabCloseScopeArg {
    Target,
    Others,
    Right,
}

impl From<TabCloseScopeArg> for TabCloseScope {
    fn from(value: TabCloseScopeArg) -> Self {
        match value {
            TabCloseScopeArg::Target => Self::Target,
            TabCloseScopeArg::Others => Self::Others,
            TabCloseScopeArg::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PaneDirectionArg {
    Left,
    Right,
    Up,
    Down,
}

impl From<PaneDirectionArg> for PaneDirection {
    fn from(value: PaneDirectionArg) -> Self {
        match value {
            PaneDirectionArg::Left => Self::Left,
            PaneDirectionArg::Right => Self::Right,
            PaneDirectionArg::Up => Self::Up,
            PaneDirectionArg::Down => Self::Down,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InputModeArg {
    Terminal,
    Agent,
}

impl From<InputModeArg> for InputMode {
    fn from(value: InputModeArg) -> Self {
        match value {
            InputModeArg::Terminal => Self::Terminal,
            InputModeArg::Agent => Self::Agent,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SizeAdjustmentArg {
    Increase,
    Decrease,
    Reset,
    Set,
}

impl From<SizeAdjustmentArg> for SizeAdjustment {
    fn from(value: SizeAdjustmentArg) -> Self {
        match value {
            SizeAdjustmentArg::Increase => Self::Increase,
            SizeAdjustmentArg::Decrease => Self::Decrease,
            SizeAdjustmentArg::Reset => Self::Reset,
            SizeAdjustmentArg::Set => Self::Set,
        }
    }
}

#[derive(Serialize)]
struct InstanceSummary {
    instance_id: String,
    pid: u32,
    channel: String,
    app_id: String,
    app_version: Option<String>,
    started_at: String,
    endpoint: Option<local_control::discovery::ControlEndpoint>,
    outside_warp_control_enabled: bool,
    actions: Vec<ActionMetadata>,
}

impl From<local_control::discovery::InstanceRecord> for InstanceSummary {
    fn from(record: local_control::discovery::InstanceRecord) -> Self {
        Self {
            instance_id: record.instance_id.0,
            pid: record.pid,
            channel: record.channel,
            app_id: record.app_id,
            app_version: record.app_version,
            started_at: record.started_at.to_rfc3339(),
            endpoint: record.endpoint,
            outside_warp_control_enabled: record.outside_warp_control_enabled,
            actions: record.actions,
        }
    }
}

#[derive(Serialize)]
struct ErrorSummary<'a> {
    ok: bool,
    error: &'a ControlError,
}

pub fn run(args: ControlArgs) -> ExitCode {
    let output_format = args.output_format;
    match run_inner(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Err(write_error) = write_control_error(&error, output_format) {
                eprintln!(
                    "error: failed to render local-control error: {}",
                    write_error.message
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn run_inner(args: ControlArgs) -> Result<(), ControlError> {
    let output_format = args.output_format;
    match args.command {
        ControlCommand::Instance(command) => run_instance_command(command, output_format),
        ControlCommand::App(command) => run_app_command(command, output_format),
        ControlCommand::Action(command) => run_action_command(command, output_format),
        ControlCommand::Window(command) => run_window_command(command, output_format),
        ControlCommand::Tab(command) => run_tab_command(command, output_format),
        ControlCommand::Pane(command) => run_pane_command(command, output_format),
        ControlCommand::Session(command) => run_session_command(command, output_format),
        ControlCommand::Block(command) => run_block_command(command, output_format),
        ControlCommand::Input(command) => run_input_command(command, output_format),
        ControlCommand::History(command) => run_history_command(command, output_format),
        ControlCommand::Theme(command) => run_theme_command(command, output_format),
        ControlCommand::Appearance(command) => run_appearance_command(command, output_format),
        ControlCommand::Setting(command) => run_setting_command(command, output_format),
        ControlCommand::File(command) => run_file_command(command, output_format),
        ControlCommand::Project(command) => run_project_command(command, output_format),
        ControlCommand::Drive(command) => run_drive_command(command, output_format),
        ControlCommand::Completions { shell } => generate_completions_to_stdout(shell),
    }
}

fn run_instance_command(
    command: InstanceCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        InstanceCommand::List => {
            let summaries = local_control::discovery::list_instances()
                .into_iter()
                .map(InstanceSummary::from)
                .collect::<Vec<_>>();
            match output_format {
                OutputFormat::Json => write_json(&summaries),
                OutputFormat::Ndjson => {
                    for summary in summaries {
                        write_json_line(&summary)?;
                    }
                    Ok(())
                }
                OutputFormat::Pretty | OutputFormat::Text => {
                    for summary in summaries {
                        let endpoint = summary
                            .endpoint
                            .as_ref()
                            .map(|endpoint| format!("{}:{}", endpoint.host, endpoint.port))
                            .unwrap_or_else(|| "outside_warp_disabled".to_owned());
                        println!(
                            "{}\tpid={}\t{}\t{}",
                            summary.instance_id, summary.pid, summary.channel, endpoint
                        );
                    }
                    Ok(())
                }
            }
        }
    }
}

fn run_app_command(command: AppCommand, output_format: OutputFormat) -> Result<(), ControlError> {
    match command {
        AppCommand::Ping(args) => {
            reject_app_target(&args, ActionKind::AppPing)?;
            run_action_with_params(
                args,
                ActionKind::AppPing,
                local_control::EmptyParams {},
                output_format,
            )
        }
        AppCommand::Version(args) => {
            reject_app_target(&args, ActionKind::AppVersion)?;
            run_action_with_params(
                args,
                ActionKind::AppVersion,
                local_control::EmptyParams {},
                output_format,
            )
        }
        AppCommand::Active(args) => {
            reject_app_target(&args, ActionKind::AppActive)?;
            run_action_with_params(
                args,
                ActionKind::AppActive,
                local_control::AppActiveParams::default(),
                output_format,
            )
        }
        AppCommand::Inspect(args) => {
            reject_app_target(&args, ActionKind::AppInspect)?;
            run_action_with_params(
                args,
                ActionKind::AppInspect,
                local_control::AppInspectParams::default(),
                output_format,
            )
        }
        AppCommand::Focus(args) => {
            reject_app_target(&args, ActionKind::AppFocus)?;
            run_action_with_params(
                args,
                ActionKind::AppFocus,
                AppFocusParams::default(),
                output_format,
            )
        }
        AppCommand::SettingsOpen(args) => {
            run_app_surface_command(args, ActionKind::AppSettingsOpen, output_format)
        }
        AppCommand::CommandPaletteOpen(args) => {
            run_app_surface_command(args, ActionKind::AppCommandPaletteOpen, output_format)
        }
        AppCommand::CommandSearchOpen(args) => {
            run_app_surface_command(args, ActionKind::AppCommandSearchOpen, output_format)
        }
        AppCommand::WarpDriveOpen(args) => {
            run_app_surface_command(args, ActionKind::AppWarpDriveOpen, output_format)
        }
        AppCommand::WarpDriveToggle(args) => {
            run_app_surface_command(args, ActionKind::AppWarpDriveToggle, output_format)
        }
        AppCommand::ResourceCenterToggle(args) => {
            run_app_surface_command(args, ActionKind::AppResourceCenterToggle, output_format)
        }
        AppCommand::AiAssistantToggle(args) => {
            run_app_surface_command(args, ActionKind::AppAiAssistantToggle, output_format)
        }
        AppCommand::CodeReviewToggle(args) => {
            run_app_surface_command(args, ActionKind::AppCodeReviewToggle, output_format)
        }
        AppCommand::VerticalTabsToggle(args) => {
            run_app_surface_command(args, ActionKind::AppVerticalTabsToggle, output_format)
        }
    }
}

fn run_action_command(
    command: ActionCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        ActionCommand::List(args) => run_action_with_params(
            args,
            ActionKind::ActionList,
            local_control::ActionListParams::default(),
            output_format,
        ),
        ActionCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::ActionGet,
            local_control::ActionGetParams {
                action: args.action,
            },
            output_format,
        ),
    }
}

fn run_window_command(
    command: WindowCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        WindowCommand::List(args) => run_action_with_params(
            args,
            ActionKind::WindowList,
            local_control::EmptyParams {},
            output_format,
        ),
        WindowCommand::Create(args) => run_action_with_params(
            args.target,
            ActionKind::WindowCreate,
            WindowCreateParams {
                profile: args.profile,
            },
            output_format,
        ),
        WindowCommand::Focus(args) => run_action_with_params(
            args,
            ActionKind::WindowFocus,
            WindowFocusParams::default(),
            output_format,
        ),
        WindowCommand::Close(args) => run_action_with_params(
            args.target,
            ActionKind::WindowClose,
            WindowCloseParams { force: args.force },
            output_format,
        ),
    }
}

fn run_tab_command(command: TabCommand, output_format: OutputFormat) -> Result<(), ControlError> {
    match command {
        TabCommand::List(args) => run_action_with_params(
            args,
            ActionKind::TabList,
            local_control::EmptyParams {},
            output_format,
        ),
        TabCommand::Create(args) => run_action_with_params(
            args,
            ActionKind::TabCreate,
            local_control::EmptyParams {},
            output_format,
        ),
        TabCommand::Activate(args) => run_action_with_params(
            args,
            ActionKind::TabActivate,
            TabActivateParams { relative: None },
            output_format,
        ),
        TabCommand::Previous(args) => {
            run_tab_activate_relative(args, TabActivationTarget::Previous, output_format)
        }
        TabCommand::Next(args) => {
            run_tab_activate_relative(args, TabActivationTarget::Next, output_format)
        }
        TabCommand::Last(args) => {
            run_tab_activate_relative(args, TabActivationTarget::Last, output_format)
        }
        TabCommand::Move(args) => run_action_with_params(
            args.target,
            ActionKind::TabMove,
            TabMoveParams {
                direction: args.direction.into(),
            },
            output_format,
        ),
        TabCommand::Rename(args) => run_action_with_params(
            args.target,
            ActionKind::TabRename,
            TabRenameParams {
                title: if args.reset { None } else { args.title },
            },
            output_format,
        ),
        TabCommand::Close(args) => run_action_with_params(
            args.target,
            ActionKind::TabClose,
            TabCloseParams {
                scope: args.scope.into(),
                force: args.force,
            },
            output_format,
        ),
    }
}

fn run_pane_command(command: PaneCommand, output_format: OutputFormat) -> Result<(), ControlError> {
    match command {
        PaneCommand::List(args) => run_action_with_params(
            args,
            ActionKind::PaneList,
            local_control::EmptyParams {},
            output_format,
        ),
        PaneCommand::Split(args) => run_action_with_params(
            args.target,
            ActionKind::PaneSplit,
            PaneSplitParams {
                direction: args.direction.into(),
                profile: args.profile,
            },
            output_format,
        ),
        PaneCommand::Focus(args) => run_action_with_params(
            args,
            ActionKind::PaneFocus,
            PaneFocusParams::default(),
            output_format,
        ),
        PaneCommand::Navigate(args) => run_action_with_params(
            args.target,
            ActionKind::PaneNavigate,
            PaneNavigateParams {
                direction: args.direction.into(),
            },
            output_format,
        ),
        PaneCommand::Close(args) => run_action_with_params(
            args.target,
            ActionKind::PaneClose,
            PaneCloseParams { force: args.force },
            output_format,
        ),
        PaneCommand::Maximize(args) => run_action_with_params(
            args.target,
            ActionKind::PaneMaximize,
            PaneMaximizeParams {
                enabled: args.enabled,
            },
            output_format,
        ),
        PaneCommand::Resize(args) => run_action_with_params(
            args.target,
            ActionKind::PaneResize,
            PaneResizeParams {
                direction: args.direction.into(),
                amount: args.amount,
            },
            output_format,
        ),
        PaneCommand::PreviousSession(args) => run_action_with_params(
            args,
            ActionKind::PaneSessionPrevious,
            local_control::EmptyParams {},
            output_format,
        ),
        PaneCommand::NextSession(args) => run_action_with_params(
            args,
            ActionKind::PaneSessionNext,
            local_control::EmptyParams {},
            output_format,
        ),
    }
}

fn run_session_command(
    command: SessionCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SessionCommand::List(args) => run_action_with_params(
            args,
            ActionKind::SessionList,
            local_control::EmptyParams {},
            output_format,
        ),
    }
}

fn run_block_command(
    command: BlockCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        BlockCommand::List(args) => run_action_with_params(
            args.target,
            ActionKind::BlockList,
            local_control::BlockListParams { limit: args.limit },
            output_format,
        ),
        BlockCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::BlockGet,
            local_control::BlockGetParams {
                block_id: args.block_id,
            },
            output_format,
        ),
    }
}

fn run_input_command(
    command: InputCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        InputCommand::Get(args) => run_action_with_params(
            args,
            ActionKind::InputGet,
            local_control::InputGetParams::default(),
            output_format,
        ),
        InputCommand::Insert(args) => run_action_with_params(
            args.target,
            ActionKind::InputInsert,
            InputInsertParams {
                text: args.text,
                replace: args.replace,
            },
            output_format,
        ),
        InputCommand::Replace(args) => run_action_with_params(
            args.target,
            ActionKind::InputReplace,
            InputReplaceParams { text: args.text },
            output_format,
        ),
        InputCommand::Clear(args) => run_action_with_params(
            args,
            ActionKind::InputClear,
            InputClearParams::default(),
            output_format,
        ),
        InputCommand::Mode(args) => run_action_with_params(
            args.target,
            ActionKind::InputModeSet,
            InputModeSetParams {
                mode: args.mode.into(),
            },
            output_format,
        ),
        InputCommand::Run(args) => run_action_with_params(
            args.target,
            ActionKind::InputRun,
            InputRunParams {
                command: args.command,
            },
            output_format,
        ),
    }
}

fn run_history_command(
    command: HistoryCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        HistoryCommand::List(args) => run_action_with_params(
            args.target,
            ActionKind::HistoryList,
            local_control::HistoryListParams { limit: args.limit },
            output_format,
        ),
    }
}

fn run_theme_command(
    command: ThemeCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        ThemeCommand::List(args) => run_action_with_params(
            args,
            ActionKind::ThemeList,
            local_control::EmptyParams {},
            output_format,
        ),
        ThemeCommand::Set(args) => run_action_with_params(
            args.target,
            ActionKind::ThemeSet,
            ThemeSetParams { name: args.name },
            output_format,
        ),
    }
}

fn run_appearance_command(
    command: AppearanceCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        AppearanceCommand::Get(args) => run_action_with_params(
            args,
            ActionKind::AppearanceGet,
            local_control::EmptyParams {},
            output_format,
        ),
        AppearanceCommand::Set(args) => run_action_with_params(
            args.target,
            ActionKind::AppearanceSet,
            AppearanceSetParams {
                theme: args.theme,
                follow_system_theme: args.follow_system_theme,
                light_theme: args.light_theme,
                dark_theme: args.dark_theme,
            },
            output_format,
        ),
        AppearanceCommand::FontSize(args) => run_action_with_params(
            args.target,
            ActionKind::AppearanceFontSize,
            AppearanceFontSizeParams {
                adjustment: args.adjustment.into(),
                value: args.value,
            },
            output_format,
        ),
        AppearanceCommand::Zoom(args) => run_action_with_params(
            args.target,
            ActionKind::AppearanceZoom,
            AppearanceZoomParams {
                adjustment: args.adjustment.into(),
                value: args.value,
            },
            output_format,
        ),
    }
}

fn run_setting_command(
    command: SettingCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        SettingCommand::List(args) => run_action_with_params(
            args,
            ActionKind::SettingList,
            local_control::SettingListParams::default(),
            output_format,
        ),
        SettingCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::SettingGet,
            local_control::SettingGetParams { key: args.key },
            output_format,
        ),
        SettingCommand::Set(args) => run_action_with_params(
            args.target,
            ActionKind::SettingSet,
            SettingSetParams {
                key: args.key,
                value: parse_json_value_or_string(args.value),
            },
            output_format,
        ),
        SettingCommand::Toggle(args) => run_action_with_params(
            args.target,
            ActionKind::SettingToggle,
            SettingToggleParams { key: args.key },
            output_format,
        ),
    }
}

fn run_file_command(command: FileCommand, output_format: OutputFormat) -> Result<(), ControlError> {
    match command {
        FileCommand::List(args) => run_action_with_params(
            args,
            ActionKind::FileList,
            local_control::FileListParams::default(),
            output_format,
        ),
        FileCommand::Open(args) => run_action_with_params(
            args.target,
            ActionKind::FileOpen,
            FileOpenParams {
                path: args.path,
                line: args.line,
                new_window: args.new_window,
            },
            output_format,
        ),
        FileCommand::Write(args) => run_action_with_params(
            args.target,
            ActionKind::FileWrite,
            FileWriteParams {
                path: args.path,
                contents: args.contents,
                create: args.create,
            },
            output_format,
        ),
        FileCommand::Delete(args) => run_action_with_params(
            args.target,
            ActionKind::FileDelete,
            FileDeleteParams {
                path: args.path,
                recursive: args.recursive,
            },
            output_format,
        ),
    }
}

fn run_project_command(
    command: ProjectCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        ProjectCommand::Active(args) => run_action_with_params(
            args,
            ActionKind::ProjectActive,
            local_control::ProjectActiveParams::default(),
            output_format,
        ),
        ProjectCommand::List(args) => run_action_with_params(
            args,
            ActionKind::ProjectList,
            local_control::ProjectListParams::default(),
            output_format,
        ),
    }
}

fn run_drive_command(
    command: DriveCommand,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match command {
        DriveCommand::List(args) => run_action_with_params(
            args.target,
            ActionKind::DriveList,
            local_control::DriveListParams {
                object_type: args.object_type.map(Into::into),
            },
            output_format,
        ),
        DriveCommand::Get(args) => run_action_with_params(
            args.target,
            ActionKind::DriveGet,
            local_control::DriveGetParams {
                object_type: args.object_type.into(),
                id: args.id,
            },
            output_format,
        ),
        DriveCommand::Create(args) => run_action_with_params(
            args.target,
            ActionKind::DriveCreate,
            DriveCreateParams {
                object_type: args.object_type.into(),
                name: args.name,
                content: parse_json_value_or_string(args.content),
            },
            output_format,
        ),
        DriveCommand::Update(args) => run_action_with_params(
            args.target,
            ActionKind::DriveUpdate,
            DriveUpdateParams {
                object_type: args.object_type.into(),
                id: args.id,
                content: parse_json_value_or_string(args.content),
            },
            output_format,
        ),
        DriveCommand::Delete(args) => run_action_with_params(
            args.target,
            ActionKind::DriveDelete,
            DriveDeleteParams {
                object_type: args.object_type.into(),
                id: args.id,
            },
            output_format,
        ),
        DriveCommand::Run(args) => run_action_with_params(
            args.target,
            ActionKind::DriveRun,
            DriveRunParams {
                object_type: args.object_type.into(),
                id: args.id,
            },
            output_format,
        ),
        DriveCommand::Insert(args) => run_action_with_params(
            args.target,
            ActionKind::DriveInsert,
            DriveInsertParams {
                object_type: args.object_type.into(),
                id: args.id,
            },
            output_format,
        ),
    }
}

fn run_action_with_params<T: Serialize>(
    args: TargetArgs,
    action: ActionKind,
    params: T,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    let records = local_control::discovery::list_instances();
    let selector = args.instance_selector();
    let target = args.target_selector();
    let instance = select_instance(&records, &selector)?;
    let mut request = RequestEnvelope::new(Action::with_params(action, params)?);
    request.target = target;
    let response = local_control::client::send_request(&instance, &request)?;
    let local_control::protocol::ControlResponse::Ok { data } = response.response else {
        return Err(ControlError::new(
            ErrorCode::Internal,
            "local-control request failed without an error payload",
        ));
    };
    match output_format {
        OutputFormat::Json => write_json(&data),
        OutputFormat::Ndjson => write_json_line(&data),
        OutputFormat::Pretty | OutputFormat::Text => write_json(&data),
    }
}

fn reject_app_target(args: &TargetArgs, action: ActionKind) -> Result<(), ControlError> {
    if args.has_app_target() {
        return Err(ControlError::new(
            ErrorCode::InvalidSelector,
            format!("{} does not accept target selectors", action.as_str()),
        ));
    }
    Ok(())
}

fn generate_completions_to_stdout(shell: Option<Shell>) -> Result<(), ControlError> {
    let shell = shell.or_else(Shell::from_env).ok_or_else(|| {
        ControlError::new(
            ErrorCode::InvalidParams,
            "could not determine shell from environment; provide a shell argument",
        )
    })?;
    let mut cmd = ControlArgs::clap_command();
    let bin_name = crate::binary_name().unwrap_or_else(|| "warpctrl".to_owned());
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}

#[cfg(test)]
fn generate_completion_string(shell: Shell) -> Result<String, ControlError> {
    let mut cmd = ControlArgs::clap_command();
    let mut output = Vec::new();
    generate(shell, &mut cmd, "warpctrl", &mut output);
    String::from_utf8(output).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to render local-control completions",
            err.to_string(),
        )
    })
}

fn write_control_error(
    error: &ControlError,
    output_format: OutputFormat,
) -> Result<(), ControlError> {
    match output_format {
        OutputFormat::Json => write_json(&ErrorSummary { ok: false, error }),
        OutputFormat::Ndjson => write_json_line(&ErrorSummary { ok: false, error }),
        OutputFormat::Pretty | OutputFormat::Text => {
            eprintln!("error: {}: {}", error.code, error.message);
            if let Some(details) = &error.details {
                eprintln!("details: {details}");
            }
            Ok(())
        }
    }
}

fn write_json(value: &impl Serialize) -> Result<(), ControlError> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer_pretty(&mut lock, value).map_err(write_error)?;
    writeln!(&mut lock).map_err(write_error)?;
    Ok(())
}

fn write_json_line(value: &impl Serialize) -> Result<(), ControlError> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value).map_err(write_error)?;
    writeln!(&mut lock).map_err(write_error)?;
    Ok(())
}

fn write_error(error: impl std::error::Error) -> ControlError {
    ControlError::with_details(
        ErrorCode::Internal,
        "failed to write local-control output",
        error.to_string(),
    )
}

#[cfg(test)]
#[path = "local_control_tests.rs"]
mod tests;
