//! Invalid browser input requests rejected before a page receives them.

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum Error {
    #[error("A page click count must be 1 or 2")]
    ClickCount,
    #[error("Page coordinates must be from 0 through 1000")]
    Coordinate,
    #[error("Page scroll deltas must be nonzero and from -10000 through 10000")]
    Scroll,
    #[error("Page input text must be 1 through 16384 UTF-8 bytes")]
    Text,
}
