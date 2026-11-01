//! Native visual-computer input for `browser click`, `type`, `key`,
//! `hotkey`, `scroll`, `drag`, and `select`. The exact tab is shown before
//! input, then AppKit sends the action to its WKWebView.
mod error;

use error::Error;

use super::{holder, page_tab};
use crate::control::{
    protocol::{
        Caller, ErrorCode, Response,
        tabs::{PAGE_COORDINATE_MAX, PAGE_INPUT_TEXT_BYTES, PAGE_WAIT_SECONDS, PageInput},
    },
    server::{Watches, socket, tabs::reveal},
};
use gpui::App;
use std::time::Duration;
type Result<T, E = Error> = std::result::Result<T, E>;

fn coordinates(input: &PageInput) -> impl Iterator<Item = u16> {
    let values = match input {
        PageInput::Click { x, y, .. }
        | PageInput::Scroll { x, y, .. }
        | PageInput::Select { x, y, .. } => [*x, *y, 0, 0],
        PageInput::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
        } => [*from_x, *from_y, *to_x, *to_y],
        PageInput::Type { .. } | PageInput::Key { .. } => [0; 4],
    };
    values.into_iter()
}

fn text(input: &PageInput) -> Option<&str> {
    match input {
        PageInput::Type { text } | PageInput::Select { text, .. } => Some(text),
        PageInput::Key { key, .. } => Some(key),
        PageInput::Click { .. } | PageInput::Scroll { .. } | PageInput::Drag { .. } => None,
    }
}

fn validate(input: &PageInput) -> Result<()> {
    if coordinates(input).any(|coordinate| coordinate > PAGE_COORDINATE_MAX) {
        return Err(Error::Coordinate);
    }
    if let Some(text) = text(input)
        && (text.is_empty() || text.len() > PAGE_INPUT_TEXT_BYTES)
    {
        return Err(Error::Text);
    }
    match input {
        PageInput::Click { count: 1 | 2, .. }
        | PageInput::Type { .. }
        | PageInput::Key { .. }
        | PageInput::Drag { .. }
        | PageInput::Select { .. } => Ok(()),
        PageInput::Scroll {
            delta_x, delta_y, ..
        } if (*delta_x != 0 || *delta_y != 0)
            && delta_x.unsigned_abs() <= 10_000
            && delta_y.unsigned_abs() <= 10_000 =>
        {
            Ok(())
        }
        PageInput::Click { .. } => Err(Error::ClickCount),
        PageInput::Scroll { .. } => Err(Error::Scroll),
    }
}

fn name(input: &PageInput) -> &'static str {
    match input {
        PageInput::Click { count: 2, .. } => "Double-clicked",
        PageInput::Click { .. } => "Clicked",
        PageInput::Type { .. } => "Typed in",
        PageInput::Key { .. } => "Pressed a key in",
        PageInput::Scroll { .. } => "Scrolled",
        PageInput::Drag { .. } => "Dragged in",
        PageInput::Select { .. } => "Selected an option in",
    }
}

pub(in crate::control::server::tabs) fn input(
    caller: Caller,
    id: u64,
    input: PageInput,
    incoming: socket::Incoming,
    watches: &mut Watches,
    cx: &mut App,
) {
    if let Err(error) = validate(&input) {
        return incoming.respond(Response::error(
            ErrorCode::InvalidRequest,
            error.to_string(),
        ));
    }
    let tab = match page_tab(id, cx) {
        Ok(tab) => tab,
        Err(response) => return incoming.respond(response),
    };
    if let Err(response) = reveal(&caller, &tab, cx) {
        return incoming.respond(response);
    }
    let tab_id = tab.id;
    watches.add(
        incoming,
        Duration::from_secs(PAGE_WAIT_SECONDS),
        move |cx| {
            let handle = holder(&caller, tab_id, cx)?;
            match handle.read_with(cx, |view, _| view.page_input(tab_id, &input)) {
                Ok(Some(Ok(()))) => Some(Response::done(format!("{} tab {id}", name(&input)))),
                Ok(Some(Err(error))) => Some(Response::error(ErrorCode::Failed, error.to_string())),
                Ok(None) | Err(_) => None,
            }
        },
        Response::error(
            ErrorCode::Timeout,
            format!("Tab {id}'s page was not on screen for input in time"),
        ),
        cx,
    );
}

#[cfg(test)]
mod tests;
