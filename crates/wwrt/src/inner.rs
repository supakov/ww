//! A view inside a view (docs/plugins.md): a view asks its host for a view of some of its bytes
//! by the plugin of their MIME type (`Host::open_inside`), keeps it, and draws it in a part of
//! its area (`Inner::ui`) under a line that names the bytes with the tools of the inner view.
//! The inner view draws with an id of its own (two views of one kind keep apart), gets the keys
//! and the mouse of its part by the rules of `crate::input`, and its requests go to the host of
//! the view outside, but a step through a folder, which an inner view has none of. A view inside
//! a view inside a view stops at `DEPTH`, so a file that holds itself cannot loop.

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;
use ww::Search;
use ww::eframe::egui;
use ww::i18n::{tr, trf};
use ww::style;

use crate::plugin::{Host, Request, View};

/// The most views inside one another under the view of a tab.
pub const DEPTH: u32 = 4;

/// The ids of the inner views, one each.
static SERIAL: AtomicU64 = AtomicU64::new(0);

/// A view of some bytes made by the plugin of their type, inside the view that asked for it.
pub struct Inner {
    /// The plugin that made the view: its index among the plugins of the window and its id.
    pub plugin: usize,
    plugin_id: String,
    plugin_name: &'static str,
    /// The name of the bytes, their MIME type and their size.
    pub name: String,
    pub mime: String,
    pub size: usize,
    view: Box<dyn View>,
    id: egui::Id,
    /// How deep the inner view is: 1 inside the view of a tab.
    depth: u32,
}

impl Host<'_> {
    /// Whether a plugin of the window opens bytes of the MIME type (a plugin names it, or a type
    /// it falls back to) and a view may hold one more inside it.
    pub fn opens(&self, mime: &str) -> bool {
        self.depth < DEPTH && self.plugins.is_some_and(|p| !p.for_mime(mime).is_empty())
    }

    /// A view of some bytes under a name by the first plugin of their MIME type that opens
    /// them, for the view of this host to keep and draw (`Inner::ui`), or why there is none.
    pub fn open_inside(&self, name: &str, mime: &str, bytes: Vec<u8>) -> Result<Inner, String> {
        if self.depth >= DEPTH {
            return Err(trf(
                "views inside views stop at a depth of {n}",
                &[("n", &DEPTH)],
            ));
        }
        let Some(plugins) = self.plugins else {
            return Err(tr("no plugin opens bytes here").to_owned());
        };
        let size = bytes.len();
        let (plugin, view) = plugins.open_bytes(name, mime, bytes, &|id| self.settings_of(id))?;
        let loaded = &plugins.loaded[plugin];
        Ok(Inner {
            plugin,
            plugin_id: loaded.id.clone(),
            plugin_name: loaded.plugin.name(),
            name: name.to_owned(),
            mime: crate::mime::base(mime),
            size,
            view,
            id: egui::Id::new(("wwrt-inner", SERIAL.fetch_add(1, Ordering::Relaxed))),
            depth: self.depth + 1,
        })
    }
}

impl Inner {
    /// The name of the plugin of the view, translated.
    pub fn plugin_name(&self) -> &'static str {
        tr(self.plugin_name)
    }

    pub fn view(&mut self) -> &mut dyn View {
        self.view.as_mut()
    }

    /// The search of the inner view, for the search of the tab.
    pub fn search(&mut self) -> Option<&mut Search> {
        self.view.search()
    }

    pub fn status(&self) -> String {
        self.view.status()
    }

    /// The line over the view: the name, the type, the size and the plugin.
    pub fn title(&self) -> String {
        trf(
            "{name}: {mime}, {size} bytes, {plugin}",
            &[
                ("name", &self.name),
                ("mime", &self.mime),
                ("size", &self.size),
                ("plugin", &self.plugin_name()),
            ],
        )
    }

    /// Draws the inner view in the rest of the area of `ui`: the line that names it with the
    /// tools of the view as buttons, then the view in a part of its own.
    pub fn ui(&mut self, ui: &mut egui::Ui, host: &mut Host) {
        let settings: Value = host.settings_of(&self.plugin_id);
        let mut own = Host {
            wrap_lines: host.wrap_lines,
            page_width: host.page_width,
            settings: &settings,
            requests: Vec::new(),
            plugins: host.plugins,
            every_settings: host.every_settings,
            depth: self.depth,
        };
        let title = self.title();
        ui.push_id(self.id, |ui| {
            // The whole part painted first: the space between the line and the view is no hole.
            let fill = ui.visuals().panel_fill;
            ui.painter()
                .rect_filled(ui.available_rect_before_wrap(), 0.0, fill);
            let p = style::palette(ui);
            egui::Frame::new()
                .fill(p.card)
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        for tool in self.view.tools() {
                            let label = tr(tool.label);
                            let button =
                                style::tool_button(ui, tool.icon, label, label, true, true, false);
                            if button.clicked() {
                                self.view.act(tool.id, &mut own);
                            }
                        }
                        ui.add(egui::Label::new(title).truncate());
                    });
                });
            self.view.ui(ui, &mut own);
        });
        host.requests.extend(
            own.requests
                .into_iter()
                .filter(|r| !matches!(r, Request::Browse(_))),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::plugin::{Format, Plugin, Plugins};

    /// A view that shows its own bytes inside itself, as a file that holds itself would.
    struct Nested(Option<Result<Inner, String>>);

    impl View for Nested {
        fn ui(&mut self, ui: &mut egui::Ui, host: &mut Host) {
            let inner = self
                .0
                .get_or_insert_with(|| host.open_inside("self", "application/x-nested", vec![1]));
            if let Ok(inner) = inner {
                inner.ui(ui, host);
            }
        }

        fn status(&self) -> String {
            match &self.0 {
                Some(Ok(inner)) => format!("in {}", inner.status()),
                Some(Err(why)) => why.clone(),
                None => String::new(),
            }
        }
    }

    struct NestedPlugin;

    static NESTED: [Format; 1] = [Format {
        name: "Nested",
        extensions: &["nested"],
        mimes: &["application/x-nested"],
    }];

    impl Plugin for NestedPlugin {
        fn name(&self) -> &'static str {
            "Nested"
        }
        fn formats(&self) -> &'static [Format] {
            &NESTED
        }
        fn open(&self, _: &Path, _: &Value) -> Result<Box<dyn View>, String> {
            Ok(Box::new(Nested(None)))
        }
        fn open_bytes(
            &self,
            _: &str,
            _: &str,
            _: Vec<u8>,
            _: &Value,
        ) -> Result<Box<dyn View>, String> {
            Ok(Box::new(Nested(None)))
        }
    }

    #[test]
    fn a_view_inside_itself_stops_at_the_depth() {
        let mut plugins = Plugins::default();
        plugins.add(PathBuf::from("nested.dll"), Box::new(NestedPlugin));
        let (every, settings) = (BTreeMap::new(), Value::Null);
        let mut view = plugins.loaded[0]
            .plugin
            .open(Path::new("a.nested"), &settings)
            .unwrap();
        let ctx = egui::Context::default();
        for _ in 0..DEPTH + 2 {
            let mut host =
                Host::new(true, ww::PageWidth::View, &settings).with_plugins(&plugins, &every);
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| view.ui(ui, &mut host));
            output.textures_delta.clear();
        }
        let status = view.status();
        assert_eq!(status.matches("in ").count(), DEPTH as usize, "{status}");
        assert!(status.ends_with(&format!("a depth of {DEPTH}")), "{status}");
        let alone = Host::new(true, ww::PageWidth::View, &settings);
        assert!(
            !alone.opens("application/x-nested"),
            "a host without the plugins opens none"
        );
    }
}
