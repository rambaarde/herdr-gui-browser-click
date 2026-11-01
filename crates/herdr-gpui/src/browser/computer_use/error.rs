//! Failures while creating or delivering one native browser input action.

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Core Graphics could not create a scroll event")]
    CoreScrollEvent,
    #[error("AppKit could not create a page key event")]
    KeyEvent,
    #[error("Browser input must run on the main thread")]
    MainThread,
    #[error("The page has no visible input area")]
    NoInputArea,
    #[error("AppKit could not create a page pointer event")]
    PointerEvent,
    #[error("AppKit could not create a page scroll event")]
    ScrollEvent,
    #[error("Core Graphics could not create a scroll source")]
    ScrollSource,
    #[error("The page is not attached to a window")]
    Unattached,
    #[error("Unknown page key")]
    UnknownKey,
}
