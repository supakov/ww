//! The tab of a theme: its name, its light or dark base, its fonts and every colour. While the
//! tab is in front the window shows the theme as it is edited. Save writes its file in the
//! folder of the themes; Save as gives it another name.

use eframe::egui;

use crate::i18n::{tr, trf};
use crate::shell::{App, ShellAct};
use crate::style;
use crate::themes::{self, Theme};

pub struct ThemeDoc {
    pub theme: Theme,
    pub dirty: bool,
    pub draft: crate::doc::DraftState,
}

impl ThemeDoc {
    pub fn new(theme: Theme) -> Self {
        let draft = crate::doc::DraftState::new(theme.path.as_deref());
        Self {
            theme,
            dirty: false,
            draft,
        }
    }

    /// The title of the tab: the name of the file of the theme, or the one its first save
    /// writes.
    pub fn title(&self) -> String {
        match &self.theme.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => format!("{}.{}", self.theme.id, themes::EXTENSION),
        }
    }
}

/// What a colour of the theme paints.
fn color_label(name: &str) -> &'static str {
    match name {
        "bg" => tr("Window"),
        "card" => tr("Cards, lists and the tree"),
        "card2" => tr("Fields and the titles of lists"),
        "border" => tr("Lines and frames"),
        "text" => tr("Text"),
        "muted" => tr("Secondary text"),
        "accent" => tr("Accent and links"),
        "hover" => tr("Under the mouse"),
        "sel" => tr("Selected row"),
        "sel_text" => tr("Text of the selected row"),
        "check" => tr("Check box turned on"),
        "check_mark" => tr("Mark of the check box"),
        "off" => tr("Disabled items and empty check boxes"),
        "warn" => tr("Warnings"),
        "ok" => tr("Success"),
        "msg_bg" => tr("Message bar"),
        "msg_fg" => tr("Text of the message bar"),
        "msg_border" => tr("Frame of the message bar"),
        "code_bg" => tr("Code on pages"),
        _ => "",
    }
}

/// A row of the colour grid: the label, the picker, the value, the button back to the base.
fn color_row(
    ui: &mut egui::Ui,
    theme: &mut Theme,
    name: &str,
    label: &str,
    shown: egui::Color32,
) -> bool {
    let mut changed = false;
    let mut c = shown;
    ui.label(label);
    if egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque)
        .changed()
    {
        theme.colors.insert(name.to_owned(), themes::color_text(c));
        changed = true;
    }
    style::copy_label(
        ui,
        egui::RichText::new(themes::color_text(c)).font(style::mono(ui, 12.0)),
        false,
    );
    if theme.colors.contains_key(name) {
        if ui
            .small_button(tr("From the base"))
            .on_hover_text(tr("Takes the colour of the light or dark base again"))
            .clicked()
        {
            theme.colors.remove(name);
            changed = true;
        }
    } else {
        ui.label(style::muted(ui, tr("as in the base"), 11.0));
    }
    ui.end_row();
    changed
}

/// A list of font families to choose from, the first entry empty for the default.
fn family_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    families: &[&'static str],
    default: &str,
) -> bool {
    let shown = if value.is_empty() {
        trf("Default ({name})", &[("name", &default)])
    } else {
        value.clone()
    };
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(200.0)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(
                    value,
                    String::new(),
                    trf("Default ({name})", &[("name", &default)]),
                )
                .changed();
            for f in families {
                changed |= ui.selectable_value(value, (*f).to_owned(), *f).changed();
            }
        });
    changed
}

/// The size of the texts of a theme, a whole number of points from [`style::SIZE_STEPS`].
pub fn size_combo(ui: &mut egui::Ui, id: &str, size: &mut f32) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{size:.0}"))
        .width(style::at_least(ui, 70.0))
        .show_ui(ui, |ui| {
            for s in style::SIZE_STEPS {
                changed |= ui.selectable_value(size, s as f32, s.to_string()).changed();
            }
        });
    changed
}

/// The fonts of a theme in a grid of three rows: the family of the texts, the size, the family
/// of code. Returns whether one changed.
pub fn fonts_grid(
    ui: &mut egui::Ui,
    id: &str,
    theme: &mut Theme,
    families: &(Vec<&'static str>, Vec<&'static str>),
) -> bool {
    let mut changed = false;
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([14.0, 8.0])
        .show(ui, |ui| {
            ui.label(tr("Font"));
            changed |= family_combo(
                ui,
                &format!("{id}-font"),
                &mut theme.font,
                &families.0,
                style::FAMILIES[0].0,
            );
            ui.end_row();
            ui.label(tr("Size"));
            changed |= size_combo(ui, &format!("{id}-size"), &mut theme.font_size);
            ui.end_row();
            ui.label(tr("Font of code and values"));
            changed |= family_combo(
                ui,
                &format!("{id}-mono"),
                &mut theme.mono_font,
                &families.1,
                style::MONO_FAMILIES[0].0,
            );
            ui.end_row();
            ui.label(tr("Font of icons"));
            changed |= family_combo(
                ui,
                &format!("{id}-icons"),
                &mut theme.icon_font,
                &style::installed_icon_families(),
                style::ICON_FAMILIES[0].0,
            );
            ui.end_row();
        });
    changed
}

/// The editor; returns whether the theme changed.
pub fn theme_view<A: App>(
    ui: &mut egui::Ui,
    doc: &mut ThemeDoc,
    families: &(Vec<&'static str>, Vec<&'static str>),
    acts: &mut Vec<A::Act>,
) -> bool {
    let mut changed = false;
    egui::ScrollArea::vertical()
        .id_salt("theme-editor")
        .auto_shrink(false)
        .show(ui, |ui| {
            let theme = &mut doc.theme;
            style::header(ui, |ui| {
                ui.label(style::strong(ui, &theme.name, 18.0));
                let place = match &theme.path {
                    Some(p) => p.display().to_string(),
                    None => trf(
                        "Embedded; the first save writes themes/{file}",
                        &[("file", &format!("{}.{}", theme.id, themes::EXTENSION))],
                    ),
                };
                ui.label(style::muted(ui, place, 12.0));
                ui.label(style::muted(
                    ui,
                    tr("While this tab is in front, the window shows the theme as it is edited."),
                    12.0,
                ));
            });
            style::block(ui, |ui| {
                style::section_title(ui, tr("Theme"));
                ui.add_space(4.0);
                egui::Grid::new("theme-main")
                    .num_columns(2)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(tr("Name"));
                        let width = ui.available_width().clamp(240.0, 560.0);
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut theme.name).desired_width(width))
                            .changed();
                        ui.end_row();
                        ui.label(tr("Base"));
                        ui.horizontal(|ui| {
                            changed |= ui.radio_value(&mut theme.dark, false, tr("Light")).changed();
                            changed |= ui.radio_value(&mut theme.dark, true, tr("Dark")).changed();
                        });
                        ui.end_row();
                    });
                ui.add_space(6.0);
                if ui
                    .button(tr("Use this theme"))
                    .on_hover_text(tr("The window keeps this theme after the tab closes; save it first to keep the changes"))
                    .clicked()
                {
                    acts.push(ShellAct::UseTheme(theme.id.clone()).into());
                }
            });
            style::block(ui, |ui| {
                style::section_title(ui, tr("Fonts"));
                ui.add_space(4.0);
                changed |= fonts_grid(ui, "theme-fonts", theme, families);
            });
            style::block(ui, |ui| {
                style::section_title(ui, tr("Colours"));
                ui.add_space(4.0);
                let palette = theme.palette();
                egui::Grid::new("theme-colors")
                    .num_columns(4)
                    .spacing([14.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (name, _) in style::COLOR_NAMES {
                            let Some(c) = palette.get(name) else {
                                continue;
                            };
                            changed |= color_row(ui, theme, name, color_label(name), c);
                        }
                        // The colours the program added, after the ones of the palette.
                        for extra in style::extra_colors() {
                            let base = if theme.dark { extra.dark } else { extra.light };
                            let c = theme
                                .colors
                                .get(extra.name)
                                .and_then(|t| themes::color(t))
                                .unwrap_or(base);
                            let label = crate::i18n::tr(extra.label);
                            changed |= color_row(ui, theme, extra.name, label, c);
                        }
                    });
            });
            ui.add_space(12.0);
        });
    if changed {
        doc.dirty = true;
    }
    changed
}
