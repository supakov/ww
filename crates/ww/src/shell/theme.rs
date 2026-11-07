//! The themes of the window: the one shown, their editor, the colours of the title bar.

use std::path::PathBuf;

use eframe::egui;

use super::{App, Shell, ShellDoc};
use crate::doc::Document;
use crate::folders::dir;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::tr;
#[cfg(target_arch = "wasm32")]
use crate::i18n::trf;
use crate::settings::ThemeChoice;
use crate::themes::{self, Theme};
use crate::themeview::{self, ThemeDoc};
use crate::{dialogs, style};

impl<A: App> Shell<A> {
    /// The id of the theme the settings choose for a side when they follow the system.
    pub fn shown_theme_id(&self, dark_mode: bool) -> String {
        if !self.settings.custom_theme.is_empty() {
            return self.settings.custom_theme.clone();
        }
        let dark = match self.settings.theme {
            ThemeChoice::System => dark_mode,
            ThemeChoice::Light => false,
            ThemeChoice::Dark => true,
        };
        if dark { "dark" } else { "light" }.to_owned()
    }

    /// The light and dark themes of the window and the preference: the theme in the editor in
    /// front, else the one of the settings, else Light and Dark as the system shows its apps.
    pub fn wanted_theme(&self) -> (Theme, Theme, egui::ThemePreference) {
        let light = themes::find(&self.themes, "light");
        let dark = themes::find(&self.themes, "dark");
        let chosen = match self.active_doc().and_then(Document::shell_doc) {
            Some(ShellDoc::Theme(d)) => Some(d.theme.clone()),
            _ => (!self.settings.custom_theme.is_empty())
                .then(|| themes::find(&self.themes, &self.settings.custom_theme)),
        };
        match chosen {
            Some(t) if t.dark => (light, t, egui::ThemePreference::Dark),
            Some(t) => (t, dark, egui::ThemePreference::Light),
            None => (light, dark, self.settings.theme.preference()),
        }
    }

    pub fn apply_visuals(&mut self, ctx: &egui::Context) {
        let wanted = self.wanted_theme();
        if self.applied_theme.as_ref() == Some(&wanted) {
            return;
        }
        let (light, dark, preference) = &wanted;
        style::apply(
            ctx,
            (light.palette(), light.fonts()),
            (dark.palette(), dark.fonts()),
        );
        style::resolve_extra(&light.colors, &dark.colors);
        ctx.set_theme(*preference);
        self.applied_theme = Some(wanted);
    }

    /// Opens the editor of a theme, or brings its tab to the front.
    pub fn edit_theme(&mut self, theme: Theme) {
        let id = theme.id.clone();
        self.show_or_open(
            |d| matches!(d.shell_doc(), Some(ShellDoc::Theme(t)) if t.theme.id == id),
            || A::shell_doc(ShellDoc::Theme(Box::new(ThemeDoc::new(theme)))),
        );
    }

    pub fn open_theme_path(&mut self, path: PathBuf) {
        match Theme::load(&path) {
            Ok(theme) => {
                self.edit_theme(theme);
                self.remember_recent(&path);
            }
            Err(e) => self.open_failed(e),
        }
    }

    /// Writes the theme of the active tab to its file, or to one the user names (a download in
    /// a browser).
    pub fn save_theme(&mut self, ask: bool) {
        let Some(ShellDoc::Theme(d)) = self.active_doc().and_then(Document::shell_doc) else {
            return;
        };
        let folder = dir("themes");
        let proposed = format!(
            "{}.{}",
            dialogs::file_stem(&d.theme.name, "theme"),
            themes::EXTENSION
        );
        #[cfg(target_arch = "wasm32")]
        if ask {
            let text = d.theme.to_json();
            match crate::store::download(&proposed, text.as_bytes(), "application/json") {
                Ok(()) => self.status = trf("Downloaded {name}", &[("name", &proposed)]),
                Err(e) => self.save_failed(e),
            }
            return;
        }
        #[cfg(target_arch = "wasm32")]
        let path = d.theme.file_in(&folder);
        #[cfg(not(target_arch = "wasm32"))]
        let path = if ask {
            let dialog = rfd::FileDialog::new()
                .add_filter(tr("Theme files"), &[themes::EXTENSION])
                .set_title(tr("Save the theme as"))
                .set_file_name(proposed);
            let dialog = if std::fs::create_dir_all(&folder).is_ok() {
                dialog.set_directory(&folder)
            } else {
                dialog
            };
            let Some(path) = dialog.save_file() else {
                return;
            };
            if path.extension().is_some() {
                path
            } else {
                path.with_extension(themes::EXTENSION)
            }
        } else {
            d.theme.file_in(&folder)
        };
        let Some(ShellDoc::Theme(d)) = self.active_doc_mut().and_then(Document::shell_doc_mut)
        else {
            return;
        };
        match d.theme.save(&path) {
            Ok(()) => {
                d.theme.id = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                d.theme.path = Some(path.clone());
                d.dirty = false;
                self.saved(&path);
                self.themes = themes::themes(&folder);
                self.remember_recent(&path);
            }
            Err(e) => self.save_failed(e),
        }
    }

    /// The settings take a theme by its id.
    pub fn use_theme(&mut self, id: String) {
        let dark = themes::find(&self.themes, &id).dark;
        self.settings.theme = if dark {
            ThemeChoice::Dark
        } else {
            ThemeChoice::Light
        };
        self.settings.custom_theme = id;
        self.settings_dirty = true;
    }

    /// The editor of the theme of the active tab.
    pub fn theme_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<A::Act>) {
        let families = style::installed_families();
        let p = style::palette(ui);
        let Some(ShellDoc::Theme(doc)) = self.active_doc_mut().and_then(Document::shell_doc_mut)
        else {
            return;
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(p.bg))
            .show(ui, |ui| {
                themeview::theme_view::<A>(ui, doc, &families, acts);
            });
    }

    /// The title bar in the colours of the theme (Windows 11; the dark frame on Windows 10),
    /// given again whenever the colours change, also when the theme follows the system.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn color_title_bar(&mut self, ui: &egui::Ui, frame: &eframe::Frame) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let v = ui.visuals();
        let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
        let colors = (
            v.dark_mode,
            rgb(v.panel_fill),
            rgb(v.text_color()),
            rgb(v.window_stroke.color),
        );
        if self.title_colors == Some(colors) {
            return;
        }
        if let Ok(handle) = frame.window_handle()
            && let RawWindowHandle::Win32(w) = handle.as_raw()
        {
            title_bar_colors(w.hwnd.get(), colors.0, colors.1, colors.2, colors.3);
            self.title_colors = Some(colors);
        }
    }
}

/// Colours the title bar of a window of this process: the dark frame for a dark theme (Windows
/// 10 and 11), and on Windows 11 the caption, its text and the border, each `[red, green,
/// blue]`. An attribute the system does not know is ignored; nothing outside the window changes.
#[cfg(not(target_arch = "wasm32"))]
fn title_bar_colors(hwnd: isize, dark: bool, caption: [u8; 3], text: [u8; 3], border: [u8; 3]) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
        };
        let rgb = |c: [u8; 3]| u32::from(c[0]) | (u32::from(c[1]) << 8) | (u32::from(c[2]) << 16);
        let values = [
            (DWMWA_USE_IMMERSIVE_DARK_MODE, u32::from(dark)),
            (DWMWA_CAPTION_COLOR, rgb(caption)),
            (DWMWA_TEXT_COLOR, rgb(text)),
            (DWMWA_BORDER_COLOR, rgb(border)),
        ];
        let window = hwnd as windows_sys::Win32::Foundation::HWND;
        for (attribute, value) in values {
            // SAFETY: the handle is a window of this process; the value is a 4-byte BOOL or
            // COLORREF that lives during the call.
            unsafe {
                DwmSetWindowAttribute(
                    window,
                    attribute as u32,
                    std::ptr::from_ref(&value).cast(),
                    4,
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, dark, caption, text, border);
    }
}
