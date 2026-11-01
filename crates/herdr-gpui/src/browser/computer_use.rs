//! Native macOS input for the visual browser-control loop. Coordinates are
//! normalized against the shown WKWebView, so screenshots work across display
//! scales without DOM inspection or JavaScript event synthesis.
#![allow(unsafe_code)]
mod error;

pub(crate) use error::Error;
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

use core_graphics::{
    display::CGDisplay,
    event::{CGEvent, CGEventType, CGMouseButton, EventField},
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::CGPoint as CorePoint,
};
use foreign_types::ForeignType as _;
use herdr_control_protocol::protocol::tabs::{PAGE_COORDINATE_MAX, PageInput, PageModifiers};
use objc2::{ClassType, MainThreadMarker, msg_send, rc::Retained};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSPoint, NSString};
use objc2_web_kit::WKWebView;
use wry::WebViewExtMacOS as _;

/// Sends one validated native input action to `view` on the main thread.
pub(crate) fn perform(view: &wry::WebView, input: &PageInput) -> Result<()> {
    let page = view.webview();
    match input {
        PageInput::Click { x, y, count } => click(&page, *x, *y, *count),
        PageInput::Type { text } => type_text(&page, text),
        PageInput::Key { key, modifiers } => press(&page, key, *modifiers),
        PageInput::Scroll {
            x,
            y,
            delta_x,
            delta_y,
        } => scroll(&page, *x, *y, *delta_x, *delta_y),
        PageInput::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
        } => drag(&page, (*from_x, *from_y), (*to_x, *to_y)),
        PageInput::Select { x, y, text } => {
            click(&page, *x, *y, 1)?;
            type_text(&page, text)?;
            press(&page, "enter", PageModifiers::default())
        }
    }
}

fn local_point(view: &WKWebView, x: u16, y: u16) -> Result<NSPoint> {
    let bounds = view.bounds();
    if bounds.size.width <= 0. || bounds.size.height <= 0. {
        return Err(Error::NoInputArea);
    }
    let scale = f64::from(PAGE_COORDINATE_MAX);
    let x = (f64::from(x) / scale * bounds.size.width).min(bounds.size.width - 0.5);
    let top = (f64::from(y) / scale * bounds.size.height).min(bounds.size.height - 0.5);
    let y = if view.isFlipped() {
        top
    } else {
        bounds.size.height - top
    };
    Ok(NSPoint::new(bounds.origin.x + x, bounds.origin.y + y))
}

fn window_point(view: &WKWebView, x: u16, y: u16) -> Result<(NSPoint, isize)> {
    let _mtm = MainThreadMarker::new().ok_or(Error::MainThread)?;
    let window = view.window().ok_or(Error::Unattached)?;
    Ok((
        view.convertPoint_toView(local_point(view, x, y)?, None),
        window.windowNumber(),
    ))
}

fn mouse_event(
    view: &WKWebView,
    kind: NSEventType,
    x: u16,
    y: u16,
    count: u8,
) -> Result<Retained<NSEvent>> {
    let (location, window_number) = window_point(view, x, y)?;
    // SAFETY: This is AppKit's documented mouse-event constructor. The live
    // WKWebView and its window stay retained by Herdr during this UI update.
    let event: Option<Retained<NSEvent>> = unsafe {
        msg_send![
            NSEvent::class(),
            mouseEventWithType: kind,
            location: location,
            modifierFlags: NSEventModifierFlags::empty(),
            timestamp: 0_f64,
            windowNumber: window_number,
            context: None::<&objc2::runtime::AnyObject>,
            eventNumber: 0_isize,
            clickCount: isize::from(count),
            pressure: if kind == NSEventType::LeftMouseUp { 0_f32 } else { 1_f32 }
        ]
    };
    event.ok_or(Error::PointerEvent)
}

fn click(view: &WKWebView, x: u16, y: u16, count: u8) -> Result<()> {
    for click in 1..=count {
        let down = mouse_event(view, NSEventType::LeftMouseDown, x, y, click)?;
        view.mouseDown(&down);
        let up = mouse_event(view, NSEventType::LeftMouseUp, x, y, click)?;
        view.mouseUp(&up);
    }
    Ok(())
}

fn drag(view: &WKWebView, from: (u16, u16), to: (u16, u16)) -> Result<()> {
    let down = mouse_event(view, NSEventType::LeftMouseDown, from.0, from.1, 1)?;
    view.mouseDown(&down);
    for step in 1_u32..=8 {
        let coordinate = |start: u16, end: u16| {
            let start = i64::from(start);
            (start + (i64::from(end) - start) * i64::from(step) / 8) as u16
        };
        let x = coordinate(from.0, to.0);
        let y = coordinate(from.1, to.1);
        let moved = mouse_event(view, NSEventType::LeftMouseDragged, x, y, 1)?;
        view.mouseDragged(&moved);
    }
    let up = mouse_event(view, NSEventType::LeftMouseUp, to.0, to.1, 1)?;
    view.mouseUp(&up);
    Ok(())
}

fn scroll(view: &WKWebView, x: u16, y: u16, delta_x: i32, delta_y: i32) -> Result<()> {
    let _mtm = MainThreadMarker::new().ok_or(Error::MainThread)?;
    let window = view.window().ok_or(Error::Unattached)?;
    let local = view.convertPoint_toView(local_point(view, x, y)?, None);
    let screen = window.convertPointToScreen(local);
    let primary_height = CGDisplay::main().bounds().size.height;
    let source =
        CGEventSource::new(CGEventSourceStateID::Private).map_err(|()| Error::ScrollSource)?;
    let event = CGEvent::new_mouse_event(
        source,
        CGEventType::MouseMoved,
        CorePoint::new(screen.x, primary_height - screen.y),
        CGMouseButton::Left,
    )
    .map_err(|()| Error::CoreScrollEvent)?;
    event.set_type(CGEventType::ScrollWheel);
    event.set_integer_value_field(EventField::SCROLL_WHEEL_EVENT_IS_CONTINUOUS, 1);
    event.set_integer_value_field(
        EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1,
        -i64::from(delta_y),
    );
    event.set_integer_value_field(
        EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2,
        -i64::from(delta_x),
    );
    // SAFETY: AppKit accepts a live CGEvent and returns a retained NSEvent or
    // nil. Dispatch stays inside this WKWebView; no system event is posted.
    let raw = event.as_ptr().cast::<std::ffi::c_void>();
    let native: Option<Retained<NSEvent>> =
        unsafe { msg_send![NSEvent::class(), eventWithCGEvent: raw] };
    let native = native.ok_or(Error::ScrollEvent)?;
    view.scrollWheel(&native);
    Ok(())
}

struct Key {
    code: u16,
    characters: String,
    unmodified: String,
}

fn key(name: &str, shift: bool) -> Option<Key> {
    let lower = name.to_ascii_lowercase();
    let (code, plain) = match lower.as_str() {
        "enter" | "return" => (36, "\r"),
        "tab" => (48, "\t"),
        "space" => (49, " "),
        "backspace" => (51, "\u{7f}"),
        "escape" | "esc" => (53, "\u{1b}"),
        "delete" => (117, "\u{f728}"),
        "home" => (115, "\u{f729}"),
        "end" => (119, "\u{f72b}"),
        "pageup" | "page_up" => (116, "\u{f72c}"),
        "pagedown" | "page_down" => (121, "\u{f72d}"),
        "left" | "arrowleft" => (123, "\u{f702}"),
        "right" | "arrowright" => (124, "\u{f703}"),
        "down" | "arrowdown" => (125, "\u{f701}"),
        "up" | "arrowup" => (126, "\u{f700}"),
        _ => {
            let mut chars = name.chars();
            let character = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let unmodified = character.to_ascii_lowercase();
            let character = if shift {
                unmodified.to_ascii_uppercase()
            } else {
                character
            };
            return Some(Key {
                code: ascii_key_code(unmodified).unwrap_or(0),
                characters: character.to_string(),
                unmodified: unmodified.to_string(),
            });
        }
    };
    Some(Key {
        code,
        characters: plain.into(),
        unmodified: plain.into(),
    })
}

fn ascii_key_code(key: char) -> Option<u16> {
    Some(match key {
        'a' => 0,
        's' => 1,
        'd' => 2,
        'f' => 3,
        'h' => 4,
        'g' => 5,
        'z' => 6,
        'x' => 7,
        'c' => 8,
        'v' => 9,
        'b' => 11,
        'q' => 12,
        'w' => 13,
        'e' => 14,
        'r' => 15,
        'y' => 16,
        't' => 17,
        '1' => 18,
        '2' => 19,
        '3' => 20,
        '4' => 21,
        '6' => 22,
        '5' => 23,
        '=' => 24,
        '9' => 25,
        '7' => 26,
        '-' => 27,
        '8' => 28,
        '0' => 29,
        ']' => 30,
        'o' => 31,
        'u' => 32,
        '[' => 33,
        'i' => 34,
        'p' => 35,
        'l' => 37,
        'j' => 38,
        '\'' => 39,
        'k' => 40,
        ';' => 41,
        '\\' => 42,
        ',' => 43,
        '/' => 44,
        'n' => 45,
        'm' => 46,
        '.' => 47,
        '`' => 50,
        _ => return None,
    })
}

fn flags(modifiers: PageModifiers) -> NSEventModifierFlags {
    let mut flags = NSEventModifierFlags::empty();
    if modifiers.command {
        flags |= NSEventModifierFlags::Command;
    }
    if modifiers.control {
        flags |= NSEventModifierFlags::Control;
    }
    if modifiers.option {
        flags |= NSEventModifierFlags::Option;
    }
    if modifiers.shift {
        flags |= NSEventModifierFlags::Shift;
    }
    flags
}

fn key_event(
    view: &WKWebView,
    kind: NSEventType,
    code: u16,
    characters: &str,
    unmodified: &str,
    modifiers: PageModifiers,
) -> Result<Retained<NSEvent>> {
    let _mtm = MainThreadMarker::new().ok_or(Error::MainThread)?;
    let number = view.window().ok_or(Error::Unattached)?.windowNumber();
    let characters = NSString::from_str(characters);
    let unmodified = NSString::from_str(unmodified);
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        kind,
        NSPoint::new(0., 0.),
        flags(modifiers),
        0.,
        number,
        None,
        &characters,
        &unmodified,
        false,
        code,
    )
    .ok_or(Error::KeyEvent)
}

fn press(view: &WKWebView, name: &str, modifiers: PageModifiers) -> Result<()> {
    let key = key(name, modifiers.shift).ok_or(Error::UnknownKey)?;
    let down = key_event(
        view,
        NSEventType::KeyDown,
        key.code,
        &key.characters,
        &key.unmodified,
        modifiers,
    )?;
    view.keyDown(&down);
    let up = key_event(
        view,
        NSEventType::KeyUp,
        key.code,
        &key.characters,
        &key.unmodified,
        modifiers,
    )?;
    view.keyUp(&up);
    Ok(())
}

fn type_text(view: &WKWebView, text: &str) -> Result<()> {
    let modifiers = PageModifiers::default();
    let down = key_event(view, NSEventType::KeyDown, 0, text, text, modifiers)?;
    view.keyDown(&down);
    let up = key_event(view, NSEventType::KeyUp, 0, text, text, modifiers)?;
    view.keyUp(&up);
    Ok(())
}

#[cfg(test)]
mod tests;
