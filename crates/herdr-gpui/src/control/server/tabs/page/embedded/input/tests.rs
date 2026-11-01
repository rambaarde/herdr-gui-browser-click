use super::*;
use crate::control::protocol::tabs::PageModifiers;

#[test]
fn accepts_each_bounded_input_shape() {
    for input in [
        PageInput::Click {
            x: 0,
            y: PAGE_COORDINATE_MAX,
            count: 2,
        },
        PageInput::Type { text: "hi".into() },
        PageInput::Key {
            key: "enter".into(),
            modifiers: PageModifiers::default(),
        },
        PageInput::Scroll {
            x: 500,
            y: 500,
            delta_x: 0,
            delta_y: 800,
        },
        PageInput::Drag {
            from_x: 100,
            from_y: 200,
            to_x: 900,
            to_y: 800,
        },
        PageInput::Select {
            x: 500,
            y: 500,
            text: "Second".into(),
        },
    ] {
        assert_eq!(validate(&input), Ok(()));
    }
}

#[test]
fn rejects_actions_that_cannot_name_safe_native_input() {
    for input in [
        PageInput::Click {
            x: PAGE_COORDINATE_MAX + 1,
            y: 0,
            count: 1,
        },
        PageInput::Click {
            x: 0,
            y: 0,
            count: 3,
        },
        PageInput::Type {
            text: String::new(),
        },
        PageInput::Scroll {
            x: 500,
            y: 500,
            delta_x: 0,
            delta_y: 0,
        },
    ] {
        assert!(validate(&input).is_err(), "{input:?}");
    }
}
