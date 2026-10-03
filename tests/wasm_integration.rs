//! WASM-specific integration tests.
//!
//! Run with: `wasm-pack test --headless --chrome`
//!
//! These tests execute inside a real browser via `wasm-bindgen-test` and
//! verify DOM interactions and WASM-specific code paths.

#![cfg(target_arch = "wasm32")]

use kilowatt_tycoon::resources::ChargerIdentityRegistry;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn console_error_panic_hook_installs_without_panic() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen_test]
fn web_sys_window_is_available() {
    let window = web_sys::window().expect("should have a global Window");
    let document = window.document().expect("Window should have a Document");
    assert!(
        !document.title().is_empty() || document.title().is_empty(),
        "document.title() should be callable"
    );
}

#[wasm_bindgen_test]
fn can_create_html_audio_element() {
    let document = web_sys::window()
        .expect("window")
        .document()
        .expect("document");
    let audio = document
        .create_element("audio")
        .expect("should create <audio> element");
    assert_eq!(audio.tag_name(), "AUDIO");
}

#[wasm_bindgen_test]
fn can_read_viewport_dimensions() {
    let window = web_sys::window().expect("window");
    let width = window
        .inner_width()
        .expect("inner_width")
        .as_f64()
        .expect("as_f64");
    let height = window
        .inner_height()
        .expect("inner_height")
        .as_f64()
        .expect("as_f64");
    assert!(
        width > 0.0,
        "viewport width should be positive, got {width}"
    );
    assert!(
        height > 0.0,
        "viewport height should be positive, got {height}"
    );
}

#[wasm_bindgen_test]
fn wasm_integration_browser_storage_persists_and_restores_registry() {
    let storage = web_sys::window()
        .expect("window")
        .local_storage()
        .expect("localStorage")
        .expect("localStorage is available");
    storage
        .remove_item("kilowatt-tycoon.charge-point-identities.v1")
        .unwrap();

    let mut registry = ChargerIdentityRegistry::load_default();
    let assigned = registry
        .assign(Some("wasm-browser-storage-regression"))
        .unwrap();
    let mut restored = ChargerIdentityRegistry::load_default();
    assert_eq!(
        restored
            .assign(Some("wasm-browser-storage-regression"))
            .unwrap(),
        assigned
    );

    storage
        .remove_item("kilowatt-tycoon.charge-point-identities.v1")
        .unwrap();
}
