//! The keys of the commands of the window, each with its text, named once. A program adds its
//! own with `Keys::new`.
//!
//! A browser keeps Ctrl+N, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+Shift+PgUp/PgDn for itself and
//! never hands them to a page, so a build for wasm32 takes Alt for them: Alt+N, Alt+W, Alt+PgDn and
//! Alt+PgUp, Alt+Shift+PgUp/PgDn. The page stops the browser from acting on the other keys of the
//! program (`crates/wwlite/web/index.html`).

use eframe::egui::{Key, KeyboardShortcut, Modifiers};

/// A keyboard shortcut with its text.
pub struct Keys {
    pub text: &'static str,
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Keys {
    pub const fn new(text: &'static str, modifiers: Modifiers, key: Key) -> Self {
        Self {
            text,
            modifiers,
            key,
        }
    }

    pub fn shortcut(&self) -> KeyboardShortcut {
        KeyboardShortcut::new(self.modifiers, self.key)
    }
}

pub const CTRL_SHIFT: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: true,
    mac_cmd: false,
    command: false,
};

/// Alt with Shift: the keys of the tabs in a browser.
#[cfg(target_arch = "wasm32")]
pub const ALT_SHIFT: Modifiers = Modifiers {
    alt: true,
    ctrl: false,
    shift: true,
    mac_cmd: false,
    command: false,
};

#[cfg(not(target_arch = "wasm32"))]
pub const NEW: Keys = Keys::new("Ctrl+N", Modifiers::CTRL, Key::N);
#[cfg(target_arch = "wasm32")]
pub const NEW: Keys = Keys::new("Alt+N", Modifiers::ALT, Key::N);
pub const OPEN: Keys = Keys::new("Ctrl+O", Modifiers::CTRL, Key::O);
pub const SAVE: Keys = Keys::new("Ctrl+S", Modifiers::CTRL, Key::S);
pub const SAVE_AS: Keys = Keys::new("Ctrl+Shift+S", CTRL_SHIFT, Key::S);
#[cfg(not(target_arch = "wasm32"))]
pub const CLOSE: Keys = Keys::new("Ctrl+W", Modifiers::CTRL, Key::W);
#[cfg(target_arch = "wasm32")]
pub const CLOSE: Keys = Keys::new("Alt+W", Modifiers::ALT, Key::W);
pub const SETTINGS: Keys = Keys::new("Ctrl+,", Modifiers::CTRL, Key::Comma);
pub const FIND: Keys = Keys::new("Ctrl+F", Modifiers::CTRL, Key::F);
#[cfg(not(target_arch = "wasm32"))]
pub const NEXT_TAB: Keys = Keys::new("Ctrl+Tab", Modifiers::CTRL, Key::Tab);
#[cfg(not(target_arch = "wasm32"))]
pub const PREVIOUS_TAB: Keys = Keys::new("Ctrl+Shift+Tab", CTRL_SHIFT, Key::Tab);
#[cfg(not(target_arch = "wasm32"))]
pub const TAB_LEFT: Keys = Keys::new("Ctrl+Shift+PgUp", CTRL_SHIFT, Key::PageUp);
#[cfg(not(target_arch = "wasm32"))]
pub const TAB_RIGHT: Keys = Keys::new("Ctrl+Shift+PgDn", CTRL_SHIFT, Key::PageDown);
#[cfg(target_arch = "wasm32")]
pub const NEXT_TAB: Keys = Keys::new("Alt+PgDn", Modifiers::ALT, Key::PageDown);
#[cfg(target_arch = "wasm32")]
pub const PREVIOUS_TAB: Keys = Keys::new("Alt+PgUp", Modifiers::ALT, Key::PageUp);
#[cfg(target_arch = "wasm32")]
pub const TAB_LEFT: Keys = Keys::new("Alt+Shift+PgUp", ALT_SHIFT, Key::PageUp);
#[cfg(target_arch = "wasm32")]
pub const TAB_RIGHT: Keys = Keys::new("Alt+Shift+PgDn", ALT_SHIFT, Key::PageDown);
