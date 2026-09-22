use crate::image_decoder::{decode_with_fallback, target_dimensions, DecodeBackend};
use std::cell::Cell;

#[test]
fn target_dimensions_preserve_landscape_and_portrait_aspect_ratios() {
    assert_eq!(target_dimensions(4000, 2000, 1440), (1440, 720));
    assert_eq!(target_dimensions(2000, 4000, 1440), (720, 1440));
    assert_eq!(target_dimensions(800, 600, 1440), (800, 600));
}

#[test]
fn decoder_uses_fallback_only_after_primary_failure() {
    let fallback_calls = Cell::new(0);
    let (value, backend) = decode_with_fallback(
        || None,
        || {
            fallback_calls.set(fallback_calls.get() + 1);
            Some("fallback")
        },
    )
    .expect("fallback should succeed");

    assert_eq!(value, "fallback");
    assert_eq!(backend, DecodeBackend::Fallback);
    assert_eq!(fallback_calls.get(), 1);
}

#[test]
fn decoder_does_not_call_fallback_after_primary_success() {
    let fallback_calls = Cell::new(0);
    let (_, backend) = decode_with_fallback(
        || Some("primary"),
        || {
            fallback_calls.set(fallback_calls.get() + 1);
            Some("fallback")
        },
    )
    .expect("primary should succeed");

    assert_eq!(backend, DecodeBackend::Primary);
    assert_eq!(fallback_calls.get(), 0);
}
