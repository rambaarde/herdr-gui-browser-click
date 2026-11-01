//! `browser ...`: page tabs, and the notes the user sends from them.
pub use herdr_control_protocol::Placement;

use super::{Args, CliError, ZoomChange};
use herdr_control_protocol::protocol::tabs::{
    PAGE_COORDINATE_MAX, PAGE_INPUT_TEXT_BYTES, PageInput, PageModifiers,
};
use std::ffi::OsString;

/// `herdr-gpui browser ...`, answered by the running app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserCommand {
    Open {
        /// A web address or a local file, told apart when it runs.
        target: String,
        workspace: Option<String>,
        focus: bool,
        /// `--beside` or `--tab`; otherwise chosen by what the page is.
        placement: Option<Placement>,
    },
    Reload,
    /// Waits up to `wait` seconds for notes; zero only checks.
    Feedback {
        wait: u64,
    },
    Skill,
    /// Prints the explainer kit's example page.
    Example,
    Help,
    List,
    Close {
        id: u64,
    },
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
    Text {
        id: u64,
    },
    Screenshot {
        id: u64,
        path: std::path::PathBuf,
    },
    /// Native input sent to one exact page tab.
    Input {
        id: u64,
        input: PageInput,
    },
    Zoom {
        id: u64,
        change: ZoomChange,
    },
}

fn utf8(value: OsString) -> Result<String, CliError> {
    value
        .into_string()
        .map_err(CliError::InvalidBrowserEncoding)
}

fn tab_id(value: Option<OsString>) -> Result<u64, CliError> {
    value
        .as_ref()
        .and_then(|value| value.to_str()?.parse().ok())
        .ok_or(CliError::InvalidValue("TAB (an ID from `browser list`)"))
}

fn coordinate(value: Option<OsString>) -> Result<u16, CliError> {
    value
        .as_ref()
        .and_then(|value| value.to_str()?.parse().ok())
        .filter(|value| *value <= PAGE_COORDINATE_MAX)
        .ok_or(CliError::InvalidValue("COORDINATE (0-1000)"))
}

fn delta(value: Option<OsString>) -> Result<i32, CliError> {
    value
        .as_ref()
        .and_then(|value| value.to_str()?.parse().ok())
        .filter(|value: &i32| value.unsigned_abs() <= 10_000)
        .ok_or(CliError::InvalidValue("DELTA (-10000 to 10000)"))
}

fn input_text(value: Option<OsString>, command: &'static str) -> Result<String, CliError> {
    let text = utf8(value.ok_or(CliError::MissingArgument {
        command,
        what: "TEXT",
    })?)?;
    if text.is_empty() || text.len() > PAGE_INPUT_TEXT_BYTES {
        return Err(CliError::InvalidValue("TEXT (1-16384 UTF-8 bytes)"));
    }
    Ok(text)
}

fn key_name(value: Option<OsString>) -> Result<String, CliError> {
    let key = utf8(value.ok_or(CliError::MissingArgument {
        command: "browser key",
        what: "KEY",
    })?)?;
    if key.is_empty() || key.len() > 32 {
        return Err(CliError::InvalidValue("KEY"));
    }
    Ok(key)
}

fn input(id: u64, input: PageInput) -> BrowserCommand {
    BrowserCommand::Input { id, input }
}

fn hotkey(id: u64, args: impl Iterator<Item = OsString>) -> Result<BrowserCommand, CliError> {
    let mut modifiers = PageModifiers::default();
    let mut key = None;
    for argument in args {
        let text = utf8(argument.clone())?;
        match text.to_ascii_lowercase().as_str() {
            "cmd" | "command" if key.is_none() => modifiers.command = true,
            "ctrl" | "control" if key.is_none() => modifiers.control = true,
            "alt" | "option" if key.is_none() => modifiers.option = true,
            "shift" if key.is_none() => modifiers.shift = true,
            _ if key.is_none() => key = Some(text),
            _ => return Err(CliError::UnexpectedArgument(argument)),
        }
    }
    if !modifiers.command && !modifiers.control && !modifiers.option && !modifiers.shift {
        return Err(CliError::InvalidValue("MODIFIER... KEY"));
    }
    Ok(input(
        id,
        PageInput::Key {
            key: key.ok_or(CliError::MissingArgument {
                command: "browser hotkey",
                what: "MODIFIER... KEY",
            })?,
            modifiers,
        },
    ))
}

/// `+`, `-`, `reset`, or a percent such as `125` or `125%`.
fn zoom_change(value: Option<OsString>) -> Result<ZoomChange, CliError> {
    let value = value.ok_or(CliError::MissingArgument {
        command: "browser zoom",
        what: "+|-|reset|PERCENT",
    })?;
    Ok(match value.to_str() {
        Some("+" | "in") => ZoomChange::In,
        Some("-" | "out") => ZoomChange::Out,
        Some("reset") => ZoomChange::Reset,
        percent => ZoomChange::Percent {
            percent: percent
                .map(|text| text.strip_suffix('%').unwrap_or(text))
                .and_then(|text| text.parse().ok())
                .filter(|percent| crate::browser::Zoom::try_from(*percent).is_ok())
                .ok_or(CliError::InvalidValue("PERCENT (50-300)"))?,
        },
    })
}

/// The rest of a command that takes no more arguments.
fn none_left(
    command: BrowserCommand,
    mut args: impl Iterator<Item = OsString>,
) -> Result<BrowserCommand, CliError> {
    match args.next() {
        None => Ok(command),
        Some(extra) if extra == "--help" || extra == "-h" => Ok(BrowserCommand::Help),
        Some(extra) => Err(CliError::UnexpectedArgument(extra)),
    }
}

pub(super) fn parse(args: &mut Args) -> Result<BrowserCommand, CliError> {
    let mut args = args.take_all();
    let command = args.next().ok_or(CliError::MissingBrowserCommand)?;
    match command.to_str() {
        Some("--help" | "-h") => return Ok(BrowserCommand::Help),
        Some("skill") => return none_left(BrowserCommand::Skill, args),
        Some("example") => return none_left(BrowserCommand::Example, args),
        Some("reload") => return none_left(BrowserCommand::Reload, args),
        Some("list") => return none_left(BrowserCommand::List, args),
        Some(verb @ ("close" | "back" | "forward" | "text")) => {
            let id = tab_id(args.next())?;
            let command = match verb {
                "close" => BrowserCommand::Close { id },
                "back" => BrowserCommand::Back { id },
                "forward" => BrowserCommand::Forward { id },
                _ => BrowserCommand::Text { id },
            };
            return none_left(command, args);
        }
        Some("navigate") => {
            let id = tab_id(args.next())?;
            let url = utf8(args.next().ok_or(CliError::MissingUrl)?)?;
            return none_left(BrowserCommand::Navigate { id, url }, args);
        }
        Some("zoom") => {
            let id = tab_id(args.next())?;
            let change = zoom_change(args.next())?;
            return none_left(BrowserCommand::Zoom { id, change }, args);
        }
        Some("screenshot") => {
            let id = tab_id(args.next())?;
            let path = args.next().ok_or(CliError::MissingArgument {
                command: "browser screenshot",
                what: "PATH",
            })?;
            return none_left(
                BrowserCommand::Screenshot {
                    id,
                    path: path.into(),
                },
                args,
            );
        }
        Some("click") => {
            let id = tab_id(args.next())?;
            let x = coordinate(args.next())?;
            let y = coordinate(args.next())?;
            let mut count = 1;
            if let Some(argument) = args.next() {
                if argument == "--double" {
                    count = 2;
                } else if argument == "--help" || argument == "-h" {
                    return Ok(BrowserCommand::Help);
                } else {
                    return Err(CliError::UnexpectedArgument(argument));
                }
            }
            return none_left(input(id, PageInput::Click { x, y, count }), args);
        }
        Some("type") => {
            let id = tab_id(args.next())?;
            let text = input_text(args.next(), "browser type")?;
            return none_left(input(id, PageInput::Type { text }), args);
        }
        Some("key") => {
            let id = tab_id(args.next())?;
            let key = key_name(args.next())?;
            return none_left(
                input(
                    id,
                    PageInput::Key {
                        key,
                        modifiers: PageModifiers::default(),
                    },
                ),
                args,
            );
        }
        Some("hotkey") => {
            let id = tab_id(args.next())?;
            return hotkey(id, args);
        }
        Some("scroll") => {
            let id = tab_id(args.next())?;
            let x = coordinate(args.next())?;
            let y = coordinate(args.next())?;
            let delta_x = delta(args.next())?;
            let delta_y = delta(args.next())?;
            return none_left(
                input(
                    id,
                    PageInput::Scroll {
                        x,
                        y,
                        delta_x,
                        delta_y,
                    },
                ),
                args,
            );
        }
        Some("drag") => {
            let id = tab_id(args.next())?;
            let from_x = coordinate(args.next())?;
            let from_y = coordinate(args.next())?;
            let to_x = coordinate(args.next())?;
            let to_y = coordinate(args.next())?;
            return none_left(
                input(
                    id,
                    PageInput::Drag {
                        from_x,
                        from_y,
                        to_x,
                        to_y,
                    },
                ),
                args,
            );
        }
        Some("select") => {
            let id = tab_id(args.next())?;
            let x = coordinate(args.next())?;
            let y = coordinate(args.next())?;
            let text = input_text(args.next(), "browser select")?;
            return none_left(input(id, PageInput::Select { x, y, text }), args);
        }
        Some("feedback") => {
            let mut wait = 0;
            while let Some(arg) = args.next() {
                match arg.to_str() {
                    Some("--help" | "-h") => return Ok(BrowserCommand::Help),
                    Some("--wait") => {
                        wait = args
                            .next()
                            .and_then(|value| value.to_str()?.parse::<u64>().ok())
                            .filter(|seconds| *seconds > 0)
                            .ok_or(CliError::InvalidWait)?;
                    }
                    _ => return Err(CliError::UnexpectedArgument(arg)),
                }
            }
            return Ok(BrowserCommand::Feedback { wait });
        }
        Some("open") => {}
        _ => return Err(CliError::UnknownBrowserCommand(command)),
    }
    let (mut target, mut workspace, mut focus) = (None, None, true);
    let mut placement = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(BrowserCommand::Help),
            Some("--no-focus") => focus = false,
            Some("--beside") => placement = Some(Placement::Beside),
            Some("--tab") => placement = Some(Placement::Tab),
            Some("--workspace") => {
                let value = args
                    .next()
                    .filter(|value| {
                        !value.is_empty() && !value.as_encoded_bytes().starts_with(b"-")
                    })
                    .ok_or(CliError::MissingWorkspace)?;
                workspace = Some(utf8(value)?);
            }
            _ if target.is_none() && !arg.as_encoded_bytes().starts_with(b"-") => {
                target = Some(utf8(arg)?);
            }
            _ => return Err(CliError::UnexpectedArgument(arg)),
        }
    }
    Ok(BrowserCommand::Open {
        target: target.ok_or(CliError::MissingUrl)?,
        workspace,
        focus,
        placement,
    })
}
