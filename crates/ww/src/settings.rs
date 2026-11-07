//! The settings of a window: `settings.json` next to the executable (in the storage of the page
//! in a browser, through `store`). The window is the only writer; a missing or unreadable file
//! means the defaults. The fields every program has are here; the fields of the program are its
//! `Extra`, flattened into the same file.

use std::path::{Path, PathBuf};

use eframe::egui;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ww_base::{jsonfile, paths};

use crate::folders::exe_dir;
use crate::store;

pub const VERSION: u32 = 1;
const MAX_BYTES: u64 = 1024 * 1024;
const RECENT: usize = 10;

/// The fields of a program in its settings file, flattened beside the ones of the window.
pub trait Extra:
    Serialize + DeserializeOwned + Default + Clone + PartialEq + std::fmt::Debug
{
    /// The `format` of the file, which names the program.
    const FORMAT: &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub fn preference(self) -> egui::ThemePreference {
        match self {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        }
    }
}

/// How wide a page of Markdown is at most (a narrower view makes it narrower): its text wraps at
/// this width (when the lines wrap), its tables and blocks of code fill it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum PageWidth {
    /// The width of the view.
    View,
    Pixels(u32),
    /// So many characters of the text of the page.
    Characters(u32),
}

/// Where the window was when it closed, in points: its position, the size of its inside, whether
/// it was maximized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowPlace {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, bound = "X: Extra")]
pub struct Settings<X: Extra> {
    pub format: String,
    pub version: u32,
    /// The language of the interface: a code, empty to follow Windows.
    pub language: String,
    pub theme: ThemeChoice,
    /// The id of a colour theme of themes/ or the embedded ones, empty for the colours of the theme.
    pub custom_theme: String,
    /// The renderer of the next start, by its id; empty, or one this executable lacks, is
    /// Automatic. `--renderer` overrides it for one start.
    pub renderer: String,
    /// The tabs with files open at the last close are opened again at the start.
    pub restore_tabs: bool,
    /// The lines of a text or a page are wrapped to the width of the view; else the view
    /// scrolls sideways.
    pub wrap_lines: bool,
    /// How wide a page of Markdown is (`table_width` in the first builds of 0.24.0).
    #[serde(alias = "table_width")]
    pub page_width: PageWidth,
    /// The window as it was at the last close, opened there again (fitted to the monitor).
    pub window: Option<WindowPlace>,
    /// The files of the tabs open at the last close, and the active one among them.
    pub open_tabs: Vec<PathBuf>,
    pub active_tab: usize,
    /// The recently opened or saved files, the newest first.
    pub recent: Vec<PathBuf>,
    pub search_history: Vec<String>,
    /// The fields of the program.
    #[serde(flatten)]
    pub extra: X,
}

impl<X: Extra> Default for Settings<X> {
    fn default() -> Self {
        Self {
            format: X::FORMAT.to_owned(),
            version: VERSION,
            language: String::new(),
            theme: ThemeChoice::System,
            custom_theme: String::new(),
            renderer: String::new(),
            restore_tabs: true,
            wrap_lines: true,
            page_width: PageWidth::View,
            window: None,
            open_tabs: Vec::new(),
            active_tab: 0,
            recent: Vec::new(),
            search_history: Vec::new(),
            extra: X::default(),
        }
    }
}

impl<X: Extra> Settings<X> {
    /// The files the settings may live in, next to the executable: the one named after the
    /// executable (`<exe>.settings.json`, so two copies of the program with different names keep
    /// their own), the one named after the program, and `settings.json` of earlier versions.
    pub fn candidates(program: &str) -> Vec<PathBuf> {
        let dir = exe_dir();
        let mut names: Vec<String> = Vec::new();
        if let Some(exe) = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()))
        {
            names.push(format!("{exe}.settings.json"));
        }
        let own = format!("{}.settings.json", program.to_lowercase());
        if !names.contains(&own) {
            names.push(own);
        }
        names.push("settings.json".to_owned());
        names.into_iter().map(|n| dir.join(n)).collect()
    }

    /// The settings of the first of `candidates` that exists, the defaults when none does, and
    /// the file to write them to: the one read, else the first candidate.
    pub fn load(program: &str) -> (Result<Self, String>, PathBuf) {
        let candidates = Self::candidates(program);
        let path = candidates
            .iter()
            .find(|p| store::exists(p))
            .cloned()
            .unwrap_or_else(|| candidates[0].clone());
        (Self::load_from(&path), path)
    }

    /// The settings of a file; the defaults when the file does not exist. The files the settings
    /// keep relative to their folder (the program folder) are absolute in memory.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        let Some(text) = store::read(path, MAX_BYTES)? else {
            return Ok(Self::default());
        };
        let mut settings: Self =
            jsonfile::parse(&text, &jsonfile::display_name(path)).map_err(|e| e.to_string())?;
        if settings.format != X::FORMAT {
            return Err(format!(
                "{}: not a {} file",
                jsonfile::display_name(path),
                X::FORMAT
            ));
        }
        let base = folder_of(path);
        for file in settings.recent.iter_mut().chain(&mut settings.open_tabs) {
            *file = paths::resolved(file, &base);
        }
        Ok(settings)
    }

    /// Writes the settings through a temporary file that replaces the target; a file inside the
    /// folder of the settings is kept relative to it, so the program folder can move.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let base = folder_of(path);
        let mut kept = self.clone();
        for file in kept.recent.iter_mut().chain(&mut kept.open_tabs) {
            *file = paths::stored(file, &base);
        }
        let mut text = serde_json::to_string_pretty(&kept).map_err(|e| e.to_string())?;
        text.push('\n');
        store::write(path, &text)
    }

    /// Puts a file first in the recent list.
    pub fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(RECENT);
    }
}

/// The folder of a file, absolute; the current folder for a bare name.
fn folder_of(path: &Path) -> PathBuf {
    let folder = path.parent().unwrap_or(Path::new(""));
    std::path::absolute(folder).unwrap_or_else(|_| folder.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    #[serde(default)]
    struct Fields {
        flag: bool,
    }

    impl Extra for Fields {
        const FORMAT: &'static str = "test-settings";
    }

    type Test = Settings<Fields>;

    #[test]
    fn settings_round_trip() {
        let dir = std::env::temp_dir().join(format!("ww-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert_eq!(Test::load_from(&path).unwrap(), Test::default());
        let mut settings = Test {
            language: "uk".to_owned(),
            theme: ThemeChoice::Dark,
            extra: Fields { flag: true },
            ..Test::default()
        };
        settings.remember(Path::new("a.txt"));
        settings.remember(Path::new("b.txt"));
        settings.remember(Path::new("a.txt"));
        settings.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"flag\": true"), "{text}");
        let loaded = Test::load_from(&path).unwrap();
        assert_eq!(loaded, settings);
        assert_eq!(
            loaded.recent,
            vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]
        );
        std::fs::write(&path, "{\"format\": \"other\"}").unwrap();
        assert!(Test::load_from(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn files_inside_the_program_folder_are_kept_relative() {
        let dir = std::env::temp_dir().join(format!("ww-relative-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let inside = dir.join("notes").join("Today.txt");
        let outside = std::env::temp_dir().join("Elsewhere.txt");
        let mut settings = Test::default();
        settings.remember(&outside);
        settings.remember(&inside);
        settings.open_tabs = vec![inside.clone()];
        settings.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let kept = Path::new(".").join("notes").join("Today.txt");
        let quoted = serde_json::to_string(&kept).unwrap();
        assert!(text.contains(&quoted), "{text}");
        assert!(!text.contains(&serde_json::to_string(&inside).unwrap()));
        assert!(text.contains(&serde_json::to_string(&outside).unwrap()));
        let moved = std::env::temp_dir().join(format!("ww-moved-{}", std::process::id()));
        std::fs::create_dir_all(&moved).unwrap();
        std::fs::copy(&path, moved.join("settings.json")).unwrap();
        let loaded = Test::load_from(&moved.join("settings.json")).unwrap();
        let there = moved.join("notes").join("Today.txt");
        assert_eq!(loaded.recent, vec![there.clone(), outside]);
        assert_eq!(loaded.open_tabs, vec![there]);
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&moved).unwrap();
    }
}
