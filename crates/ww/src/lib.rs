//! WW (W#): a window with tabs for programs that analyse and manage a system. A program
//! implements [`App`] and runs through [`run`]; the shell ([`Shell`], [`Window`]) owns the tabs,
//! the settings, the themes, the session, the chrome and the acts of the window ([`ShellAct`]).
//! The look of the window (`style`), the themes and fonts (`themes`, `themeview`), the
//! translations (`i18n`), the painted tree and list (`tree`, `list`), the modal window and the
//! side panel (`widgets`), the Markdown viewer (`mdview`), the renderers (`renderer`), the
//! settings file (`settings`), the drafts and the log (`session`), the files of the window
//! natively or in the storage of a browser (`store`), the file dialogs (`dialogs`), the folders
//! next to the executable (`folders`), the search of a tab and the trait of a document (`doc`),
//! the files embedded in the executable or a plugin (`embedded`, `embed!`), a question over the
//! window (`question`), the keys and the items of the command bar (`keys`, `commands`), the icons
//! (`icons`), a text file in the editor of egui (`textdoc`), the start in a browser (`web`, wasm32
//! only).

pub mod coloricons;
pub mod commands;
pub mod dialogs;
pub mod doc;
pub mod embedded;
pub mod folders;
pub mod i18n;
pub mod icons;
pub mod keys;
pub mod list;
pub mod markdown;
pub mod mdview;
pub mod question;
pub mod renderer;
#[cfg(not(target_arch = "wasm32"))]
pub mod run;
pub mod sample;
pub mod session;
pub mod settings;
pub mod shell;
pub mod store;
pub mod style;
pub mod textdoc;
pub mod themes;
pub mod themeview;
pub mod tree;
#[cfg(target_arch = "wasm32")]
pub mod web;
pub mod widgets;

// The crates a program on WW names in its own code: egui through eframe, the file dialogs.
/// The folder of the sources of W#, for the tests of a program that checks the translations of
/// its texts (its checkout of git when W# is a git dependency).
pub const SOURCES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

pub use eframe;
pub use rfd;
pub use ww_base::clock;

pub use commands::{Command, Item, MenuButton};
pub use doc::{Document, Draft, DraftState, Scrolled, Search, matches_words, remember_search};
pub use embedded::Embedded;
pub use question::Question;
#[cfg(not(target_arch = "wasm32"))]
pub use run::{run, smoke_window};
pub use settings::{Extra, PageWidth, Settings, ThemeChoice, WindowPlace};
pub use shell::{App, Job, Messages, Nav, PageDoc, SettingsDoc, Shell, ShellAct, ShellDoc, Window};
pub use themeview::ThemeDoc;
