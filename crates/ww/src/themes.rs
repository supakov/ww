//! Themes: the colours and the fonts of the window. Light and Dark are embedded in WW, a program
//! embeds more (`install`); a file
//! `themes/<id>.wwtheme` next to the executable (in the storage of the page in a browser, through
//! `store`) adds a theme, or replaces the embedded one of its id. A theme names a light or dark
//! base and the colours it changes; a colour it leaves out keeps the base. Files `<id>.json` of
//! the first version are read too.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use ww_base::jsonfile;

use crate::store;
use crate::style::{self, Fonts, Palette};

pub const FORMAT: &str = "winweaver-theme";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "wwtheme";
const MAX_BYTES: u64 = 64 * 1024;

/// The themes of WW: Light and Dark.
pub const EMBEDDED: [(&str, &str); 2] = [
    ("dark", include_str!("../data/themes/dark.wwtheme")),
    ("light", include_str!("../data/themes/light.wwtheme")),
];

/// The themes the program embeds besides the ones of WW, set once at the start.
static EXTRA: std::sync::RwLock<&'static [(&'static str, &'static str)]> =
    std::sync::RwLock::new(&[]);

/// Names the themes the program embeds besides Light and Dark: the id and the file.
pub fn install(extra: &'static [(&'static str, &'static str)]) {
    if let Ok(mut e) = EXTRA.write() {
        *e = extra;
    }
}

/// The embedded themes: the ones of WW and the ones of the program.
fn embedded_list() -> Vec<(&'static str, &'static str)> {
    let extra = EXTRA.read().map(|e| *e).unwrap_or(&[]);
    EMBEDDED.iter().chain(extra.iter()).copied().collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// The name of its file without the extension, or of the embedded theme.
    #[serde(skip)]
    pub id: String,
    /// The file it was read from; `None` for an embedded theme no file replaces.
    #[serde(skip)]
    pub path: Option<PathBuf>,
    pub format: String,
    pub version: u32,
    pub name: String,
    pub dark: bool,
    /// The family of the texts, empty for Segoe UI ([`style::FAMILIES`]).
    pub font: String,
    /// The size of the texts in points, 13 by default.
    pub font_size: f32,
    /// The family of code and values, empty for Consolas ([`style::MONO_FAMILIES`]).
    pub mono_font: String,
    /// The family of the icons, empty for Segoe UI Emoji ([`style::ICON_FAMILIES`]): the icons of
    /// the command bar and the tabs in colour.
    pub icon_font: String,
    /// Colours as `#rrggbb` by their names ([`style::COLOR_NAMES`]).
    pub colors: BTreeMap<String, String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            id: String::new(),
            path: None,
            format: FORMAT.to_owned(),
            version: VERSION,
            name: String::new(),
            dark: false,
            font: String::new(),
            font_size: style::BASE_SIZE,
            mono_font: String::new(),
            icon_font: String::new(),
            colors: BTreeMap::new(),
        }
    }
}

impl Theme {
    /// Reads a theme; a size outside [`style::SIZES`] is brought into it.
    pub fn parse(text: &str, name: &str) -> Result<Self, String> {
        let mut theme: Theme = jsonfile::parse(text, name).map_err(|e| e.to_string())?;
        if theme.format.is_empty() {
            theme.format = FORMAT.to_owned();
        }
        if theme.format != FORMAT {
            return Err(format!(
                "{name}: not a WinWeaver theme (format {})",
                theme.format
            ));
        }
        if theme.version > VERSION {
            return Err(format!(
                "{name}: theme version {} is newer than this program knows ({VERSION})",
                theme.version
            ));
        }
        if !theme.font_size.is_finite() {
            theme.font_size = style::BASE_SIZE;
        }
        theme.font_size = theme
            .font_size
            .clamp(*style::SIZES.start(), *style::SIZES.end());
        theme.colors.retain(|_, v| color(v).is_some());
        Ok(theme)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let name = jsonfile::display_name(path);
        let text = store::read(path, MAX_BYTES)?.ok_or_else(|| format!("{name}: not found"))?;
        let mut theme = Self::parse(&text, &name)?;
        theme.id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        theme.path = Some(path.to_owned());
        if theme.name.is_empty() {
            theme.name = theme.id.clone();
        }
        Ok(theme)
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into());
        text.push('\n');
        text
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        store::write(path, &self.to_json())
    }

    /// The embedded theme of an id.
    pub fn embedded(id: &str) -> Option<Self> {
        embedded_list()
            .into_iter()
            .find(|e| e.0 == id)
            .and_then(|(id, text)| {
                Self::parse(text, id).ok().map(|mut t| {
                    t.id = id.to_owned();
                    t
                })
            })
    }

    pub fn palette(&self) -> Palette {
        Palette::from_colors(self.dark, &self.colors)
    }

    pub fn fonts(&self) -> Fonts {
        Fonts {
            family: self.font.clone(),
            size: self.font_size,
            mono: self.mono_font.clone(),
            icons: self.icon_font.clone(),
        }
    }

    /// The file a save writes: its own, or `<id>.wwtheme` in the folder of the themes for an
    /// embedded one.
    pub fn file_in(&self, dir: &Path) -> PathBuf {
        self.path
            .clone()
            .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
            .unwrap_or_else(|| dir.join(format!("{}.{EXTENSION}", self.id)))
    }
}

/// The embedded themes and the files of the folder, by id; a file with the id of an embedded
/// theme replaces it, a `.wwtheme` file a `.json` one of the same id.
pub fn themes(dir: &Path) -> Vec<Theme> {
    let mut out: Vec<Theme> = embedded_list()
        .into_iter()
        .filter_map(|(id, _)| Theme::embedded(id))
        .collect();
    let mut paths = store::list(dir, &["json", EXTENSION]);
    paths.sort_by_key(|p| p.extension().is_some_and(|e| e == EXTENSION));
    for path in paths {
        if let Ok(t) = Theme::load(&path) {
            out.retain(|o| o.id != t.id);
            out.push(t);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The theme of an id, from the folder or embedded; Light when there is none.
pub fn find(list: &[Theme], id: &str) -> Theme {
    list.iter()
        .find(|t| t.id == id)
        .cloned()
        .or_else(|| Theme::embedded(id))
        .or_else(|| Theme::embedded("light"))
        .unwrap_or_default()
}

/// A colour written as `#rrggbb`.
pub fn color(text: &str) -> Option<Color32> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// A colour as `#rrggbb`.
pub fn color_text(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_themes_load_with_their_colours_and_fonts() {
        let list = themes(&std::env::temp_dir().join("winweaver-no-themes"));
        // Light and Dark of WW; a program adds its own with `install`.
        assert_eq!(list.len(), 2);
        let light = list.iter().find(|t| t.id == "light").unwrap();
        assert_eq!(
            light.palette(),
            Palette::LIGHT,
            "Light is the palette of the window"
        );
        let dark = list.iter().find(|t| t.id == "dark").unwrap();
        assert_eq!(dark.palette(), Palette::DARK);
        assert!(dark.dark);
        assert_eq!(color("#00ff41"), Some(Color32::from_rgb(0, 255, 65)));
        assert_eq!(color("00ff41"), None);
        assert_eq!(color_text(Color32::from_rgb(0, 255, 65)), "#00ff41");
    }

    #[test]
    fn a_theme_file_replaces_the_embedded_one_and_saves_back() {
        let dir = std::env::temp_dir().join(format!("winweaver-themes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("latte.json"),
            r##"{"name": "Old latte", "base": "clam", "dark": false, "font": "", "colors": {"select": "#8c5e3c", "bad": "red"}}"##,
        )
        .unwrap();
        let list = themes(&dir);
        let old = list.iter().find(|t| t.id == "latte").unwrap();
        assert_eq!(old.name, "Old latte", "a file of the first version is read");
        assert_eq!(old.font_size, 13.0);
        assert!(!old.colors.contains_key("bad"));
        let mut mine = old.clone();
        mine.name = "My latte".into();
        mine.font = "Verdana".into();
        mine.font_size = 48.0;
        let path = mine.file_in(&dir);
        assert_eq!(path, dir.join("latte.wwtheme"));
        mine.save(&path).unwrap();
        let back = find(&themes(&dir), "latte");
        assert_eq!(back.name, "My latte");
        assert_eq!(back.font, "Verdana");
        assert_eq!(back.font_size, 36.0, "the size is kept in its range");
        assert_eq!(back.path.as_deref(), Some(path.as_path()));
        assert!(Theme::parse(r#"{"format": "winweaver-profile"}"#, "x").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
