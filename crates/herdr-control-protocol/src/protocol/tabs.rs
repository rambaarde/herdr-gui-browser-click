//! The app's own tabs: pages, code, reviews, the orchestrator, and the
//! Devices overview, which Herdr does not know about.
use serde::{Deserialize, Serialize};

/// How long a page may take to hand back its text or picture.
pub const PAGE_WAIT_SECONDS: u64 = 15;
/// How long the window may take to switch to the caller's workspace before
/// opening a tab there.
pub const ARRIVE_WAIT_SECONDS: u64 = 5;
/// Browser input coordinates span this square, from the page's top-left.
/// Keeping them independent of display scale lets agents act on Retina and
/// resized windows from the same screenshot.
pub const PAGE_COORDINATE_MAX: u16 = 1_000;
/// Largest text one browser input request accepts.
pub const PAGE_INPUT_TEXT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum TabOp {
    /// Every app tab, or only browser pages.
    List {
        #[serde(default)]
        pages: bool,
    },
    Focus {
        id: u64,
    },
    Close {
        id: u64,
    },
    /// Load a web address in an existing page tab.
    Navigate {
        id: u64,
        url: String,
    },
    Back {
        id: u64,
    },
    Forward {
        id: u64,
    },
    /// The page's visible text, as the user would select it.
    Text {
        id: u64,
    },
    /// A PNG of the page, written to `path`.
    Screenshot {
        id: u64,
        path: String,
    },
    /// Native input into the page. Coordinates use [`PAGE_COORDINATE_MAX`].
    Input {
        id: u64,
        input: PageInput,
    },
    /// How large a page tab draws its page.
    Zoom {
        id: u64,
        change: ZoomChange,
    },
    /// A read-only code tab on a local file, at a 1-based line.
    Code {
        path: String,
        #[serde(default)]
        line: Option<u32>,
    },
    /// The file in the user's terminal editor, in a new pane.
    Edit {
        path: String,
        #[serde(default)]
        line: Option<u32>,
    },
    Devices,
    /// The orchestrator of the caller's workspace, or of the checkout at
    /// `repo`.
    Orchestrator {
        #[serde(default)]
        repo: Option<String>,
    },
    /// The review of the caller's workspace's uncommitted changes.
    Review,
    Px0,
}

impl TabOp {
    pub fn wait_seconds(&self) -> u64 {
        match self {
            Self::Text { .. } | Self::Screenshot { .. } | Self::Input { .. } => PAGE_WAIT_SECONDS,
            Self::Code { .. }
            | Self::Edit { .. }
            | Self::Devices
            | Self::Orchestrator { .. }
            | Self::Review
            | Self::Px0 => ARRIVE_WAIT_SECONDS,
            Self::List { .. }
            | Self::Focus { .. }
            | Self::Close { .. }
            | Self::Navigate { .. }
            | Self::Back { .. }
            | Self::Forward { .. }
            | Self::Zoom { .. } => 0,
        }
    }
}

/// A step in or out, back to 100%, or a zoom of its own, in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoomChange {
    In,
    Out,
    Reset,
    Percent { percent: u16 },
}

/// Modifier keys held around one native page key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageModifiers {
    #[serde(default)]
    pub command: bool,
    #[serde(default)]
    pub control: bool,
    #[serde(default)]
    pub option: bool,
    #[serde(default)]
    pub shift: bool,
}

/// One native visual-computer input action in a page tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case", deny_unknown_fields)]
pub enum PageInput {
    /// Click once or twice at a normalized page coordinate.
    Click {
        x: u16,
        y: u16,
        #[serde(default = "single_click")]
        count: u8,
    },
    /// Insert text at the page's current native insertion point.
    Type { text: String },
    /// Press and release one key with optional modifiers.
    Key {
        key: String,
        #[serde(default)]
        modifiers: PageModifiers,
    },
    /// Scroll at a normalized page coordinate; positive deltas move content
    /// right and down.
    Scroll {
        x: u16,
        y: u16,
        delta_x: i32,
        delta_y: i32,
    },
    /// Drag from one normalized page coordinate to another.
    Drag {
        from_x: u16,
        from_y: u16,
        to_x: u16,
        to_y: u16,
    },
    /// Choose an option after opening the control at a coordinate.
    Select { x: u16, y: u16, text: String },
}

const fn single_click() -> u8 {
    1
}

impl TabKind {
    /// The kind as `--json` spells it, for the text listing.
    pub fn name(self) -> &'static str {
        match self {
            Self::Blank => "blank",
            Self::Web => "web",
            Self::File => "file",
            Self::Code => "code",
            Self::Review => "review",
            Self::Orchestrator => "orchestrator",
            Self::Devices => "devices",
            Self::CoreTerminal => "core_terminal",
            Self::Editor => "editor",
        }
    }

    /// Whether a page shows it: what `browser list` keeps.
    pub fn is_page(self) -> bool {
        matches!(self, Self::Web | Self::File)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabKind {
    Blank,
    Web,
    File,
    Code,
    Review,
    Orchestrator,
    Devices,
    /// A herdr-core (Herdr 2) terminal the app draws.
    CoreTerminal,
    /// The workspace's VS Code tab.
    Editor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: u64,
    pub kind: TabKind,
    pub title: String,
    /// The address, file, or checkout the tab shows.
    pub location: Option<String>,
    pub workspace: String,
    /// The host the workspace belongs to: a local socket or an SSH host.
    pub host: String,
    /// The pane that opened the tab, when an agent did.
    pub opened_by: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Tabs {
        tabs: Vec<TabInfo>,
    },
    Opened {
        id: u64,
        workspace: String,
    },
    Text {
        text: String,
    },
    /// The zoom a page tab draws at now, in percent.
    Zoom {
        id: u64,
        percent: u16,
    },
}

#[cfg(test)]
mod tests;
