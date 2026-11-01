use super::*;

#[test]
fn translates_named_and_printing_keys() {
    let enter = key("enter", false).unwrap();
    assert_eq!((enter.code, enter.characters.as_str()), (36, "\r"));

    let shifted = key("p", true).unwrap();
    assert_eq!(
        (
            shifted.code,
            shifted.characters.as_str(),
            shifted.unmodified.as_str(),
        ),
        (35, "P", "p")
    );
    assert!(key("not-a-key", false).is_none());
}

#[test]
fn builds_exact_modifier_flags() {
    let modifiers = PageModifiers {
        command: true,
        control: false,
        option: true,
        shift: true,
    };
    let actual = flags(modifiers);
    assert!(actual.contains(NSEventModifierFlags::Command));
    assert!(actual.contains(NSEventModifierFlags::Option));
    assert!(actual.contains(NSEventModifierFlags::Shift));
    assert!(!actual.contains(NSEventModifierFlags::Control));
}
