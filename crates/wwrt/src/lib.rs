//! wwrt: the runtime the programs with plugins share (docs/plugins.md). One DLL holds egui,
//! eframe, W# and the interface of the plugins; the executable and every plugin link it instead of
//! copies of their own, so a plugin is its own code alone and draws in the window of the program
//! with the egui of the program. std is the `std-<hash>.dll` of the toolchain beside it.
//!
//! A program or a plugin depends on wwrt alone and takes every crate wwrt holds through it
//! (`wwrt::ww`, `wwrt::egui`, `wwrt::serde`, `wwrt::serde_json`, `wwrt::windows`, `wwrt::image`): a dependency
//! of its own on one of them would be a second copy of that crate, which rustc refuses. A derive
//! of serde names its crate: `#[serde(crate = "wwrt::serde")]`.

#[cfg(feature = "kittest")]
pub use egui_kittest;
pub use image;
pub use serde;
pub use serde_json;
pub use sha2;
#[cfg(windows)]
pub use windows;
pub use ww;
pub use ww::eframe;
pub use ww::eframe::egui;

/// The folder of the sources of wwrt, for the tests of a program that translates its texts.
pub const SOURCES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

pub mod blocks;
pub mod inner;
pub mod input;
pub mod mime;
pub mod plugin;
pub mod stable;
#[doc(hidden)]
pub mod testing;
pub mod trust;
