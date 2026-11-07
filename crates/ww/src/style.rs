//! The look of the window: the palette of light and dark, the fonts of the theme from the fonts of
//! Windows (Segoe UI and Consolas by default), the spacing, and the small painted widgets egui has
//! no style for: the buttons of the command bar, icon buttons, cards, section titles and chips.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Margin, RichText, Sense, Shape,
    Stroke, StrokeKind,
};

use crate::icons;

/// The colours of the interface, the tokens of the palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// The window behind the cards.
    pub bg: Color32,
    pub card: Color32,
    /// Fields and the inside of cards.
    pub card2: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub hover: Color32,
    /// The selected row.
    pub sel: Color32,
    /// The text of the selected row.
    pub sel_text: Color32,
    /// A check box that is on, and its mark.
    pub check: Color32,
    pub check_mark: Color32,
    /// Disabled items and the frame of an empty check box.
    pub off: Color32,
    pub warn: Color32,
    pub ok: Color32,
    /// The message bar.
    pub msg_bg: Color32,
    pub msg_fg: Color32,
    pub msg_border: Color32,
    /// The code on a page: the blocks and the words of code.
    pub code_bg: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// A colour a program adds to the themes beyond the ones of the palette: its name in a theme
/// file, its label in the theme editor (translated), its value on the light and on the dark
/// base. A theme file overrides it by its name like any colour.
pub struct ExtraColor {
    pub name: &'static str,
    pub label: &'static str,
    pub light: Color32,
    pub dark: Color32,
}

static EXTRA_COLORS: std::sync::RwLock<&'static [ExtraColor]> = std::sync::RwLock::new(&[]);
/// The extra colours of the themes shown now: the name, the light and the dark value.
static EXTRA_VALUES: std::sync::RwLock<Vec<(String, Color32, Color32)>> =
    std::sync::RwLock::new(Vec::new());

/// The colours a program adds to the themes, named once at the start.
pub fn install_colors(list: &'static [ExtraColor]) {
    *EXTRA_COLORS.write().unwrap_or_else(|e| e.into_inner()) = list;
}

/// The colours the program added.
pub fn extra_colors() -> &'static [ExtraColor] {
    *EXTRA_COLORS.read().unwrap_or_else(|e| e.into_inner())
}

/// The value of an added colour on a base, before a theme overrides it.
pub fn extra_default(name: &str, dark: bool) -> Option<Color32> {
    extra_colors()
        .iter()
        .find(|c| c.name == name)
        .map(|c| if dark { c.dark } else { c.light })
}

/// The added colours of the two themes the window shows, from their files; `apply` of the
/// themes calls it.
pub fn resolve_extra(
    light: &std::collections::BTreeMap<String, String>,
    dark: &std::collections::BTreeMap<String, String>,
) {
    let values = extra_colors()
        .iter()
        .map(|c| {
            let of = |map: &std::collections::BTreeMap<String, String>, base: Color32| {
                map.get(c.name)
                    .and_then(|t| crate::themes::color(t))
                    .unwrap_or(base)
            };
            (c.name.to_owned(), of(light, c.light), of(dark, c.dark))
        })
        .collect();
    *EXTRA_VALUES.write().unwrap_or_else(|e| e.into_inner()) = values;
}

/// An added colour of the theme the window shows; the text colour for a name nobody added.
pub fn extra(ui: &egui::Ui, name: &str) -> Color32 {
    let dark = ui.visuals().dark_mode;
    EXTRA_VALUES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(n, _, _)| n == name)
        .map(|(_, l, d)| if dark { *d } else { *l })
        .unwrap_or_else(|| ui.visuals().text_color())
}

/// The names of the colours of a theme file, in the order of the theme editor, with the names of
/// the first version each one also takes.
pub const COLOR_NAMES: [(&str, &[&str]); 19] = [
    ("bg", &["background"]),
    ("card", &["field"]),
    ("card2", &["button", "field"]),
    ("border", &[]),
    ("text", &["foreground"]),
    ("muted", &[]),
    ("accent", &["link"]),
    ("hover", &["button_active", "button"]),
    ("sel", &["select"]),
    ("sel_text", &["select_foreground"]),
    ("check", &["check_on"]),
    ("check_mark", &[]),
    ("off", &["check_border", "disabled"]),
    ("warn", &["warning"]),
    ("ok", &[]),
    ("msg_bg", &[]),
    ("msg_fg", &[]),
    ("msg_border", &[]),
    ("code_bg", &[]),
];

impl Palette {
    pub const LIGHT: Palette = Palette {
        dark: false,
        bg: hex(0xf3f3f3),
        card: hex(0xffffff),
        card2: hex(0xf9f9f9),
        border: hex(0xe3e3e3),
        text: hex(0x1b1b1b),
        muted: hex(0x6d6d6d),
        accent: hex(0x0067c0),
        hover: hex(0xececec),
        sel: hex(0xd9e7f5),
        sel_text: hex(0x1b1b1b),
        check: hex(0x0067c0),
        check_mark: hex(0xffffff),
        off: hex(0x8d8d8d),
        warn: hex(0xc66900),
        ok: hex(0x0f7b0f),
        msg_bg: hex(0xfff4e5),
        msg_fg: hex(0x5a3b00),
        msg_border: hex(0xf0d9b5),
        code_bg: hex(0xeeeeee),
    };

    pub const DARK: Palette = Palette {
        dark: true,
        bg: hex(0x202020),
        card: hex(0x2b2b2b),
        card2: hex(0x272727),
        border: hex(0x3b3b3b),
        text: hex(0xf0f0f0),
        muted: hex(0xa3a3a3),
        accent: hex(0x4cc2ff),
        hover: hex(0x353535),
        sel: hex(0x2f4257),
        sel_text: hex(0xf0f0f0),
        check: hex(0x4cc2ff),
        check_mark: hex(0xffffff),
        off: hex(0x7a7a7a),
        warn: hex(0xffb15c),
        ok: hex(0x6ccb5f),
        msg_bg: hex(0x433418),
        msg_fg: hex(0xffd9a0),
        msg_border: hex(0x5c4620),
        code_bg: hex(0x1c1c1c),
    };

    fn slot(&mut self, name: &str) -> Option<&mut Color32> {
        Some(match name {
            "bg" => &mut self.bg,
            "card" => &mut self.card,
            "card2" => &mut self.card2,
            "border" => &mut self.border,
            "text" => &mut self.text,
            "muted" => &mut self.muted,
            "accent" => &mut self.accent,
            "hover" => &mut self.hover,
            "sel" => &mut self.sel,
            "sel_text" => &mut self.sel_text,
            "check" => &mut self.check,
            "check_mark" => &mut self.check_mark,
            "off" => &mut self.off,
            "warn" => &mut self.warn,
            "ok" => &mut self.ok,
            "msg_bg" => &mut self.msg_bg,
            "msg_fg" => &mut self.msg_fg,
            "msg_border" => &mut self.msg_border,
            "code_bg" => &mut self.code_bg,
            _ => return None,
        })
    }

    /// A colour by its name in [`COLOR_NAMES`].
    pub fn get(&self, name: &str) -> Option<Color32> {
        let mut copy = *self;
        copy.slot(name).map(|c| *c)
    }

    /// The palette of a colour theme: its light or dark base with the colours it names. The
    /// names of the first version (background, foreground, field, select...) are understood too.
    pub fn from_colors(dark: bool, colors: &std::collections::BTreeMap<String, String>) -> Self {
        let mut p = if dark { Self::DARK } else { Self::LIGHT };
        let base_text = p.text;
        let base_sel_text = p.sel_text;
        for (name, old) in COLOR_NAMES {
            let found = std::iter::once(name)
                .chain(old.iter().copied())
                .find_map(|n| colors.get(n).and_then(|t| crate::themes::color(t)));
            if let (Some(c), Some(slot)) = (found, p.slot(name)) {
                *slot = c;
            }
        }
        if p.sel_text == base_sel_text && p.text != base_text {
            p.sel_text = p.text;
        }
        p
    }
}

/// The fonts of a theme: the family of the texts, its size, the family of code and values, the
/// family of the icons.
#[derive(Debug, Clone, PartialEq)]
pub struct Fonts {
    pub family: String,
    pub size: f32,
    pub mono: String,
    pub icons: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            family: String::new(),
            size: BASE_SIZE,
            mono: String::new(),
            icons: String::new(),
        }
    }
}

impl Fonts {
    /// The icons of the command bar and the tabs are pictures in the colours of Segoe UI Emoji.
    pub fn color_icons(&self) -> bool {
        self.icons.is_empty() || self.icons.eq_ignore_ascii_case(ICON_FAMILIES[0].0)
    }
}

/// The text size the layout is drawn for; a theme with another size scales the texts.
pub const BASE_SIZE: f32 = 13.0;
/// The sizes a theme may have: a file outside the range is brought into it.
pub const SIZES: std::ops::RangeInclusive<f32> = 10.0..=36.0;
/// The sizes the settings and the theme editor offer: by one up to 20, then the steps of
/// large displays and of people with poor sight.
pub const SIZE_STEPS: [u32; 17] = [
    10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, 24, 26, 28, 32, 36,
];

/// The families of texts the program knows in the fonts folder of Windows: the name, the file
/// of the regular cut and the file of the bold or semibold cut for titles.
pub const FAMILIES: [(&str, &str, &str); 8] = [
    ("Segoe UI", "segoeui.ttf", "seguisb.ttf"),
    ("Arial", "arial.ttf", "arialbd.ttf"),
    ("Calibri", "calibri.ttf", "calibrib.ttf"),
    ("Georgia", "georgia.ttf", "georgiab.ttf"),
    ("Tahoma", "tahoma.ttf", "tahomabd.ttf"),
    ("Trebuchet MS", "trebuc.ttf", "trebucbd.ttf"),
    ("Verdana", "verdana.ttf", "verdanab.ttf"),
    ("Consolas", "consola.ttf", "consolab.ttf"),
];

/// The families of code and values: the name and the file.
pub const MONO_FAMILIES: [(&str, &str); 4] = [
    ("Consolas", "consola.ttf"),
    ("Cascadia Mono", "CascadiaMono.ttf"),
    ("Courier New", "cour.ttf"),
    ("Lucida Console", "lucon.ttf"),
];

/// The families of the icons: Segoe UI Emoji (the icons of the command bar and the tabs in
/// colour) and Segoe UI Symbol (all in the colour of the text).
pub const ICON_FAMILIES: [(&str, &str); 2] = [
    ("Segoe UI Emoji", "seguiemj.ttf"),
    ("Segoe UI Symbol", "seguisym.ttf"),
];

/// Segoe UI Symbol: the arrows, triangles, stars and marks the texts lack, before the fonts of
/// egui.
const SYMBOLS: &str = "seguisym.ttf";

fn fonts_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
        .join("Fonts")
}

/// The families of texts and of code this Windows has.
pub fn installed_families() -> (Vec<&'static str>, Vec<&'static str>) {
    let dir = fonts_dir();
    (
        FAMILIES
            .iter()
            .filter(|f| dir.join(f.1).is_file())
            .map(|f| f.0)
            .collect(),
        MONO_FAMILIES
            .iter()
            .filter(|f| dir.join(f.1).is_file())
            .map(|f| f.0)
            .collect(),
    )
}

/// The families of the icons this Windows has.
pub fn installed_icon_families() -> Vec<&'static str> {
    let dir = fonts_dir();
    ICON_FAMILIES
        .iter()
        .filter(|f| dir.join(f.1).is_file())
        .map(|f| f.0)
        .collect()
}

/// The visuals of egui from a palette.
pub fn visuals(p: &Palette) -> egui::Visuals {
    let mut v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let r = CornerRadius::same(6);
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.faint_bg_color = p.card2;
    v.extreme_bg_color = p.card2;
    v.code_bg_color = p.code_bg;
    v.text_edit_bg_color = Some(p.card2);
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.sel;
    v.selection.stroke = Stroke::new(1.0, p.sel_text);
    v.warn_fg_color = p.warn;
    v.widgets.noninteractive.bg_fill = p.card;
    v.widgets.noninteractive.weak_bg_fill = p.card;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = p.card;
    v.widgets.inactive.weak_bg_fill = p.card;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.active.bg_fill = p.sel;
    v.widgets.active.weak_bg_fill = p.sel;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.open.bg_fill = p.hover;
    v.widgets.open.weak_bg_fill = p.hover;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.border);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
        w.fg_stroke = Stroke::new(1.0, p.text);
        w.expansion = 0.0;
    }
    v
}

fn palettes_id() -> egui::Id {
    egui::Id::new("winweaver-palettes")
}

fn fonts_id() -> egui::Id {
    egui::Id::new("winweaver-fonts")
}

/// The fonts in use for light and dark, once [`install_fonts`] ran.
fn installed_id() -> egui::Id {
    egui::Id::new("winweaver-fonts-installed")
}

/// The name of the family of texts of a side: `light` or `dark`, with `-strong` for titles and
/// `-mono` for code.
fn family_name(dark: bool, kind: &str) -> FontFamily {
    let side = if dark { "dark" } else { "light" };
    FontFamily::Name(format!("{side}{kind}").into())
}

/// The font definitions for the fonts of both sides; the fonts of egui stay as the fallback
/// (symbols, emoji).
fn definitions(light: &Fonts, dark: &Fonts) -> egui::FontDefinitions {
    let dir = fonts_dir();
    let read = |name: &str| {
        std::fs::read(dir.join(name))
            .ok()
            .filter(|b| b.len() < 32 * 1024 * 1024)
    };
    let mut defs = egui::FontDefinitions::default();
    let proportional = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let monospace = defs
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let mut loaded: std::collections::BTreeSet<String> = Default::default();
    let mut load = |defs: &mut egui::FontDefinitions, file: &str| -> Option<String> {
        let key = format!("file:{}", file.to_ascii_lowercase());
        if loaded.contains(&key) {
            return Some(key);
        }
        let bytes = read(file)?;
        defs.font_data
            .insert(key.clone(), Arc::new(egui::FontData::from_owned(bytes)));
        loaded.insert(key.clone());
        Some(key)
    };
    for (dark, fonts) in [(false, light), (true, dark)] {
        let family = FAMILIES
            .iter()
            .find(|f| f.0.eq_ignore_ascii_case(&fonts.family))
            .unwrap_or(&FAMILIES[0]);
        let mono = MONO_FAMILIES
            .iter()
            .find(|f| f.0.eq_ignore_ascii_case(&fonts.mono))
            .map(|f| f.1)
            .or_else(|| {
                FAMILIES
                    .iter()
                    .find(|f| f.0.eq_ignore_ascii_case(&fonts.mono))
                    .map(|f| f.1)
            })
            .unwrap_or(MONO_FAMILIES[0].1);
        let regular = load(&mut defs, family.1).or_else(|| load(&mut defs, FAMILIES[0].1));
        let bold = load(&mut defs, family.2).or_else(|| load(&mut defs, FAMILIES[0].2));
        let code = load(&mut defs, mono);
        let icon_file = ICON_FAMILIES
            .iter()
            .find(|f| f.0.eq_ignore_ascii_case(&fonts.icons))
            .unwrap_or(&ICON_FAMILIES[0])
            .1;
        // The icons come after the text and before the symbols: a sign of the text font stays.
        let icons = load(&mut defs, icon_file);
        let symbols = load(&mut defs, SYMBOLS);
        let list = |first: Vec<Option<String>>, rest: &Vec<String>| -> Vec<String> {
            let mut keys: Vec<String> = Vec::new();
            for key in first
                .into_iter()
                .flatten()
                .chain(icons.clone())
                .chain(symbols.clone())
                .chain(rest.iter().cloned())
            {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            keys
        };
        defs.families.insert(
            family_name(dark, ""),
            list(vec![regular.clone()], &proportional),
        );
        defs.families.insert(
            family_name(dark, "-strong"),
            list(vec![bold, regular], &proportional),
        );
        defs.families
            .insert(family_name(dark, "-mono"), list(vec![code], &monospace));
    }
    let first: Vec<String> = ["file:segoeui.ttf".to_owned(), format!("file:{SYMBOLS}")]
        .into_iter()
        .filter(|k| loaded.contains(k))
        .collect();
    let family = defs.families.entry(FontFamily::Proportional).or_default();
    for (i, k) in first.into_iter().enumerate() {
        family.insert(i, k);
    }
    defs
}

/// Loads the default fonts of the theme (Segoe UI and Consolas) from the fonts folder of Windows
/// when they are there, and gives the families of both sides their names. Call it before the
/// first frame; without it the window keeps the fonts of egui and the sizes of the layout.
pub fn install_fonts(ctx: &egui::Context) {
    let fonts = Fonts::default();
    ctx.set_fonts(definitions(&fonts, &fonts));
    ctx.data_mut(|d| {
        d.insert_temp(installed_id(), (fonts.clone(), fonts.clone()));
        d.insert_temp(fonts_id(), (fonts.clone(), fonts));
    });
}

/// Sets the visuals of both themes, their fonts and text sizes and the spacing, and remembers the
/// palettes for the painted widgets. The fonts change only after [`install_fonts`].
pub fn apply(ctx: &egui::Context, light: (Palette, Fonts), dark: (Palette, Fonts)) {
    ctx.set_visuals_of(egui::Theme::Light, visuals(&light.0));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&dark.0));
    let installed = ctx.data(|d| d.get_temp::<(Fonts, Fonts)>(installed_id()));
    let named = installed.is_some();
    if let Some(now) = installed
        && (now.0 != light.1 || now.1 != dark.1)
    {
        ctx.set_fonts(definitions(&light.1, &dark.1));
        ctx.data_mut(|d| d.insert_temp(installed_id(), (light.1.clone(), dark.1.clone())));
    }
    for (theme, is_dark, fonts) in [
        (egui::Theme::Light, false, &light.1),
        (egui::Theme::Dark, true, &dark.1),
    ] {
        let scale = fonts.size / BASE_SIZE;
        let (body, mono) = if named {
            (family_name(is_dark, ""), family_name(is_dark, "-mono"))
        } else {
            (FontFamily::Proportional, FontFamily::Monospace)
        };
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = egui::vec2(6.0, 4.0);
            s.spacing.button_padding = egui::vec2(10.0, 4.0);
            s.spacing.interact_size.y = (24.0 * scale).max(24.0);
            s.spacing.menu_margin = Margin::same(6);
            // Scroll bars that stay visible while the content is larger than the view, the
            // handle in the colour of the text: the colours of the widgets of a theme are too
            // close to the colour of the track to be seen.
            let mut scroll = egui::style::ScrollStyle::solid();
            scroll.foreground_color = true;
            s.spacing.scroll = scroll;
            s.spacing.window_margin = Margin::same(10);
            for (style, size, family) in [
                (egui::TextStyle::Body, 13.0, &body),
                (egui::TextStyle::Button, 13.0, &body),
                (egui::TextStyle::Small, 11.0, &body),
                (egui::TextStyle::Heading, 18.0, &body),
                (egui::TextStyle::Monospace, 12.0, &mono),
            ] {
                s.text_styles
                    .insert(style, FontId::new(size * scale, family.clone()));
            }
        });
    }
    ctx.data_mut(|d| {
        d.insert_temp(palettes_id(), (light.0, dark.0));
        d.insert_temp(fonts_id(), (light.1, dark.1));
    });
}

/// The palette of the theme the window shows now.
pub fn palette(ui: &egui::Ui) -> Palette {
    let (light, dark) = ui
        .ctx()
        .data(|d| d.get_temp::<(Palette, Palette)>(palettes_id()))
        .unwrap_or((Palette::LIGHT, Palette::DARK));
    if ui.visuals().dark_mode { dark } else { light }
}

/// The fonts of the theme the window shows now.
fn fonts(ui: &egui::Ui) -> Fonts {
    ui.ctx()
        .data(|d| d.get_temp::<(Fonts, Fonts)>(fonts_id()))
        .map(|(l, d)| if ui.visuals().dark_mode { d } else { l })
        .unwrap_or_default()
}

/// A size of the layout in the text size of the theme.
pub fn size(ui: &egui::Ui, size: f32) -> f32 {
    size * fonts(ui).size / BASE_SIZE
}

/// A size of the layout in the text size of the theme, never below the size of the layout: the
/// room of the chrome (buttons, tabs, icons) grows with a large text and keeps its place with a
/// small one.
pub fn at_least(ui: &egui::Ui, at: f32) -> f32 {
    size(ui, at).max(at)
}

/// The height of a row of a tree or a list: 28 points, more for a large text size.
pub fn row_height(ui: &egui::Ui) -> f32 {
    at_least(ui, 28.0).round()
}

fn named(ui: &egui::Ui) -> bool {
    ui.ctx()
        .data(|d| d.get_temp::<(Fonts, Fonts)>(installed_id()))
        .is_some()
}

/// The font of the texts of a size of the layout.
pub fn font(ui: &egui::Ui, at: f32) -> FontId {
    let family = if named(ui) {
        family_name(ui.visuals().dark_mode, "")
    } else {
        FontFamily::Proportional
    };
    FontId::new(size(ui, at), family)
}

/// The font of code and values of a size of the layout.
pub fn mono(ui: &egui::Ui, at: f32) -> FontId {
    let family = if named(ui) {
        family_name(ui.visuals().dark_mode, "-mono")
    } else {
        FontFamily::Monospace
    };
    FontId::new(size(ui, at), family)
}

/// The font of titles (the bold or semibold cut) of a size of the layout.
pub fn strong_font(ui: &egui::Ui, at: f32) -> FontId {
    let family = if named(ui) {
        family_name(ui.visuals().dark_mode, "-strong")
    } else {
        FontFamily::Proportional
    };
    FontId::new(size(ui, at), family)
}

/// Text in the font of titles.
pub fn strong(ui: &egui::Ui, text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(strong_font(ui, size))
}

/// A small triangle that points down, for split buttons.
fn caret(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let s = 3.0;
    painter.add(Shape::convex_polygon(
        vec![
            center + egui::vec2(-s, -s * 0.5),
            center + egui::vec2(s, -s * 0.5),
            center + egui::vec2(0.0, s * 0.7),
        ],
        color,
        Stroke::NONE,
    ));
}

/// A button of the command bar: the icon above the label, or the icon alone when `compact`;
/// `split` adds the caret of a button with a menu.
pub fn tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    hint: &str,
    enabled: bool,
    compact: bool,
    split: bool,
) -> egui::Response {
    let p = palette(ui);
    let label_font = font(ui, 11.0);
    // The room of the button grows with the text size of the theme.
    let room = |at: f32| at_least(ui, at);
    let width = if compact {
        room(if split { 40.0 } else { 32.0 })
    } else {
        let text = ui
            .painter()
            .layout_no_wrap(label.to_owned(), label_font.clone(), p.text)
            .size()
            .x;
        (text + room(20.0) + if split { room(12.0) } else { 0.0 }).max(room(58.0))
    };
    let height = if compact { room(28.0) } else { room(44.0) };
    let (icon_top, label_bottom, caret_right, shift) = (
        room(15.0),
        room(10.0),
        room(8.0),
        if split { room(6.0) } else { 0.0 },
    );
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if enabled && response.hovered() {
            painter.rect_filled(rect, CornerRadius::same(6), p.hover);
        }
        if response.has_focus() {
            painter.rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(1.5, p.accent),
                StrokeKind::Inside,
            );
        }
        let color = if enabled { p.text } else { p.off };
        let icon_y = if compact {
            rect.center().y
        } else {
            rect.top() + icon_top
        };
        paint_icon(
            ui,
            painter,
            egui::pos2(rect.center().x - shift, icon_y),
            icon,
            if compact { 15.0 } else { 17.0 },
            color,
            enabled,
        );
        if !compact {
            painter.text(
                egui::pos2(rect.center().x - shift, rect.bottom() - label_bottom),
                Align2::CENTER_CENTER,
                label,
                label_font,
                color,
            );
        }
        if split {
            caret(
                painter,
                egui::pos2(rect.right() - caret_right, icon_y),
                p.muted,
            );
        }
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(hint)
}

/// Paints an icon centred on a point: a picture in the colours of its font when the theme draws
/// the icons in colour (faded when `enabled` is off), else its glyph in `color`. `size` is the
/// size of the text of the icon in points of the layout.
pub fn paint_icon(
    ui: &egui::Ui,
    painter: &egui::Painter,
    center: egui::Pos2,
    icon: &str,
    size: f32,
    color: Color32,
    enabled: bool,
) {
    let at = self::size(ui, size);
    if fonts(ui).color_icons() {
        let side = (at * 1.3).round();
        let px = (side * ui.ctx().pixels_per_point()).round() as u32;
        if let Some(texture) = crate::coloricons::texture(ui.ctx(), icon, px, color) {
            let rect = egui::Rect::from_center_size(center, egui::vec2(side, side));
            let tint = if enabled {
                Color32::WHITE
            } else {
                Color32::from_white_alpha(110)
            };
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture.id(), rect, uv, tint);
            return;
        }
    }
    painter.text(center, Align2::CENTER_CENTER, icon, font(ui, size), color);
}

/// A small square button with an icon, such as back, forward, the star or the search.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    enabled: bool,
    color: Option<Color32>,
) -> egui::Response {
    let p = palette(ui);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let side = egui::vec2(at_least(ui, 30.0), at_least(ui, 28.0));
    let (rect, response) = ui.allocate_exact_size(side, sense);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if enabled && response.hovered() {
            painter.rect_filled(rect, CornerRadius::same(6), p.hover);
        }
        if response.has_focus() {
            painter.rect_stroke(
                rect,
                CornerRadius::same(6),
                Stroke::new(1.5, p.accent),
                StrokeKind::Inside,
            );
        }
        let c = if enabled {
            color.unwrap_or(p.muted)
        } else {
            p.off.gamma_multiply(0.6)
        };
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            font(ui, 15.0),
            c,
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(label)
}

/// A frameless text button with the hover background, such as a part of the breadcrumb.
pub fn flat_button(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    ui.add(
        egui::Button::new(text)
            .frame(false)
            .min_size(egui::vec2(0.0, 24.0)),
    )
}

/// A card: the white rounded box of the detail panel.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = palette(ui);
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// The head of a page: the card colour across the width, a line under it.
pub fn header(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let p = palette(ui);
    egui::Frame::new()
        .fill(p.card)
        .inner_margin(Margin {
            left: 18,
            right: 18,
            top: 14,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.painter().hline(
        ui.min_rect().x_range(),
        ui.cursor().top(),
        Stroke::new(1.0, p.border),
    );
}

/// A card of a page, with its margins.
pub fn block(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .outer_margin(Margin {
            left: 18,
            right: 18,
            top: 10,
            bottom: 0,
        })
        .show(ui, |ui| {
            card(ui, add);
        });
}

/// The title of a section in capitals, small and muted.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    let p = palette(ui);
    ui.label(
        RichText::new(text.to_uppercase())
            .font(strong_font(ui, 11.0))
            .color(p.muted),
    );
}

/// A rounded label of a fact, such as the mechanism or the phase of a rule.
pub fn chip(ui: &mut egui::Ui, text: &str, accent: bool) -> egui::Response {
    let p = palette(ui);
    egui::Frame::new()
        .fill(if accent { p.sel } else { p.hover })
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(size(ui, 11.0)).color(if accent {
                p.text
            } else {
                p.muted
            }))
        })
        .inner
}

/// Text the reader can select, with a menu on the right button: copy the selection, select all,
/// copy all. It wraps at the width of the panel and never changes.
pub fn selectable_text(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, text: &str) {
    let id = ui.make_persistent_id(id);
    let mut shown = text;
    let output = egui::TextEdit::multiline(&mut shown)
        .id(id)
        .frame(egui::Frame::NONE)
        .desired_width(f32::INFINITY)
        .desired_rows(1)
        .margin(Margin::ZERO)
        .show(ui);
    let selected: String = output
        .cursor_range
        .map(|r| {
            let r = r.as_sorted_char_range();
            text.chars()
                .skip(r.start.0)
                .take(r.end.0.saturating_sub(r.start.0))
                .collect()
        })
        .unwrap_or_default();
    let galley = output.galley.clone();
    output.response.context_menu(|ui| {
        use crate::i18n::tr;
        use crate::shell::menu_entry;
        if menu_entry(ui, icons::CLIPBOARD, tr("Copy"), "", !selected.is_empty()) {
            ui.ctx().copy_text(selected.clone());
        }
        if menu_entry(ui, icons::SELECT_ALL, tr("Select all"), "", true) {
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                state
                    .cursor
                    .set_char_range(Some(egui::text_selection::CCursorRange::select_all(
                        &galley,
                    )));
                state.store(ui.ctx(), id);
            }
            ui.memory_mut(|m| m.request_focus(id));
        }
        if menu_entry(ui, icons::COPY_ALL, tr("Copy all"), "", true) {
            ui.ctx().copy_text(text.to_owned());
        }
    });
}

/// A text that can be selected with the mouse, with a menu on the right button that copies it.
pub fn copy_label(ui: &mut egui::Ui, text: RichText, wrap: bool) -> egui::Response {
    let plain = text.text().to_owned();
    let mut label = egui::Label::new(text).selectable(true);
    if wrap {
        label = label.wrap();
    }
    let response = ui.add(label);
    response.context_menu(|ui| {
        let copy = crate::i18n::tr("Copy");
        if crate::shell::menu_entry(ui, icons::CLIPBOARD, copy, "", true) {
            ui.ctx().copy_text(plain.clone());
        }
    });
    response
}

/// Muted text of a size.
pub fn muted(ui: &egui::Ui, text: impl Into<String>, at: f32) -> RichText {
    RichText::new(text)
        .size(size(ui, at))
        .color(palette(ui).muted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_of_a_theme_replace_the_base() {
        let mut colors = std::collections::BTreeMap::new();
        colors.insert("background".to_owned(), "#000000".to_owned());
        colors.insert("accent".to_owned(), "#00ff41".to_owned());
        let p = Palette::from_colors(true, &colors);
        assert_eq!(p.bg, Color32::from_rgb(0, 0, 0));
        assert_eq!(p.accent, Color32::from_rgb(0, 255, 65));
        assert_eq!(p.card, Palette::DARK.card);
        assert!(visuals(&p).dark_mode);
    }
}
