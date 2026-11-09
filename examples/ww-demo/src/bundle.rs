//! The plugin of bundles the program makes itself (`Plugins::add`): a `.wwbundle` file is a head
//! of text that names its parts (a name, a MIME type and a length each) and their bytes after it.
//! Its view lists the parts in a tree and shows the part chosen inside itself by the plugin of its
//! type (`Host::open_inside`, `wwrt::inner`); the context menu of a row (`ww::tree::show_with_menu`)
//! opens the part in a tab of its own or with any plugin (Open with), and the view offers the part
//! chosen to the bar and to the menu of the window. A part is a `BlockFile::slice` of the bundle:
//! the plugins read it through the cache of the blocks, never a copy.
//!
//! ```text
//! WWBUNDLE 1
//! hello.hello application/x-ww-hello 30
//! gradient.bin application/octet-stream 4096
//!
//! <the 30 bytes of the first part><the 4096 bytes of the second>
//! ```

use std::path::Path;

use wwrt::blocks::BlockFile;
use wwrt::egui;
use wwrt::inner::Inner;
use wwrt::menus::{self, Choice};
use wwrt::plugin::{Format, Host, Offer, Plugin, Request, Tool, View};
use wwrt::serde_json::Value;
use wwrt::ww::Search;
use wwrt::ww::i18n::{tr, trf};
use wwrt::ww::icons;
use wwrt::ww::shell::{menu_entry, sub_menu};
use wwrt::ww::tree::{self, Row, Scroll};

pub const MIME: &str = "application/x-ww-bundle";
/// The first word of a bundle.
const MAGIC: &[u8] = b"WWBUNDLE ";
/// The most bytes of the head and the most parts a bundle has.
const MAX_HEAD: usize = 64 * 1024;
const MAX_PARTS: usize = 1000;

static FORMATS: [Format; 1] = [Format {
    name: "Bundles of parts",
    extensions: &["wwbundle"],
    mimes: &[MIME],
}];

/// A part of a bundle: its name, its type and its bytes, a part of the bundle.
pub struct Part {
    pub name: String,
    pub mime: String,
    pub bytes: BlockFile,
}

/// The parts of a bundle, or why it is none.
pub fn parse(file: &BlockFile) -> Result<Vec<Part>, String> {
    let head = file.read(0, MAX_HEAD).map_err(|e| e.to_string())?;
    if !head.starts_with(MAGIC) {
        return Err(tr("not a bundle: it does not start with WWBUNDLE").to_owned());
    }
    let Some(end) = head.windows(2).position(|w| w == b"\n\n") else {
        return Err(tr("the head of the bundle has no end").to_owned());
    };
    let text = std::str::from_utf8(&head[..end])
        .map_err(|_| tr("the head of the bundle is not UTF-8").to_owned())?;
    let mut offset = end as u64 + 2;
    let mut parts = Vec::new();
    for (n, line) in text.lines().enumerate().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [name, mime, length] = fields[..] else {
            return Err(trf(
                "line {n} of the head is not a part",
                &[("n", &(n + 1))],
            ));
        };
        let length: u64 = length
            .parse()
            .map_err(|_| trf("line {n} of the head is not a part", &[("n", &(n + 1))]))?;
        if offset + length > file.len() {
            return Err(trf("the part {name} goes past the end", &[("name", &name)]));
        }
        if parts.len() == MAX_PARTS {
            return Err(trf("more than {n} parts", &[("n", &MAX_PARTS)]));
        }
        parts.push(Part {
            name: name.to_owned(),
            mime: mime.to_owned(),
            bytes: file.slice(offset, length),
        });
        offset += length;
    }
    Ok(parts)
}

pub struct BundlePlugin;

impl BundlePlugin {
    fn view(file: &BlockFile) -> Result<Box<dyn View>, String> {
        let parts = parse(file)?;
        Ok(Box::new(BundleView {
            parts,
            selected: 0,
            inner: None,
        }))
    }
}

impl Plugin for BundlePlugin {
    fn name(&self) -> &'static str {
        "Bundles of parts"
    }

    fn formats(&self) -> &'static [Format] {
        &FORMATS
    }

    fn open(&self, path: &Path, _settings: &Value) -> Result<Box<dyn View>, String> {
        let file = BlockFile::open(path).map_err(|e| e.to_string())?;
        Self::view(&file)
    }

    fn opens_bytes(&self) -> bool {
        true
    }

    /// A bundle of some bytes: one embedded in the program, or a part of another bundle.
    fn open_bytes(
        &self,
        _name: &str,
        _mime: &str,
        bytes: BlockFile,
        _settings: &Value,
    ) -> Result<Box<dyn View>, String> {
        Self::view(&bytes)
    }

    fn sniff(&self, head: &[u8]) -> Option<&'static str> {
        head.starts_with(MAGIC).then_some(MIME)
    }
}

/// What the context menu of a row asks for.
enum Pick {
    Tab(usize),
    With(usize, Choice),
}

pub struct BundleView {
    parts: Vec<Part>,
    selected: usize,
    /// The view of the part chosen by the plugin of its type, with the index of the part, or why
    /// there is none.
    inner: Option<(usize, Result<Inner, String>)>,
}

impl BundleView {
    /// The request of a part in a tab of its own, by the plugin of its type.
    fn in_a_tab(&self, i: usize) -> Option<Request> {
        let part = self.parts.get(i)?;
        Some(Request::OpenBytes {
            name: part.name.clone(),
            mime: part.mime.clone(),
            bytes: part.bytes.clone(),
        })
    }

    /// The rows of the tree: one a part, with its size and type.
    fn rows(&self) -> Vec<Row<usize>> {
        self.parts
            .iter()
            .enumerate()
            .map(|(i, p)| Row {
                key: i,
                depth: 0,
                title: p.name.clone(),
                right: trf("{n} bytes", &[("n", &p.bytes.len())]),
                check: tree::Check::None,
                open: None,
                muted: false,
                icon: Some(icons::PAGE),
                switch: None,
            })
            .collect()
    }
}

impl View for BundleView {
    fn ui(&mut self, ui: &mut egui::Ui, host: &mut Host) {
        let plugins = host.plugins;
        let mut pick = None;
        let rows = self.rows();
        let parts = &self.parts;
        let id = ui.id().with("bundle-parts");
        let mut chosen = None;
        egui::Panel::left(ui.id().with("bundle-list"))
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| {
                let mut menu = |ui: &mut egui::Ui, key: &usize| {
                    if menu_entry(ui, icons::OPEN, tr("Open in a tab"), "", true) {
                        pick = Some(Pick::Tab(*key));
                    }
                    if let (Some(plugins), Some(part)) = (plugins, parts.get(*key)) {
                        let choices = plugins.choices_for_bytes(Some(&part.mime));
                        let picked = sub_menu(ui, icons::GO_TO, tr("Open with"), |ui| {
                            menus::open_with(ui, plugins, &choices)
                        });
                        if let Some(Some(choice)) = picked {
                            pick = Some(Pick::With(*key, choice));
                        }
                    }
                };
                let out = tree::show_with_menu(
                    ui,
                    id,
                    &rows,
                    Some(self.selected),
                    Scroll::None,
                    &mut menu,
                );
                for event in out.events {
                    if let tree::Event::Select(k) = event {
                        chosen = Some(k);
                    }
                }
            });
        if let Some(k) = chosen {
            self.selected = k;
        }
        match pick {
            Some(Pick::Tab(i)) => host.requests.extend(self.in_a_tab(i)),
            Some(Pick::With(i, choice)) => {
                if let Some(part) = self.parts.get(i) {
                    host.requests.push(Request::OpenWith {
                        name: part.name.clone(),
                        plugin: choice.plugin,
                        mime: choice.mime.to_owned(),
                        bytes: part.bytes.clone(),
                    });
                }
            }
            None => {}
        }
        if self.inner.as_ref().is_none_or(|(i, _)| *i != self.selected)
            && let Some(part) = self.parts.get(self.selected)
        {
            let inner = host.open_inside(&part.name, &part.mime, part.bytes.clone());
            self.inner = Some((self.selected, inner));
        }
        egui::CentralPanel::default().show(ui, |ui| match &mut self.inner {
            Some((_, Ok(inner))) => inner.ui(ui, host),
            Some((_, Err(why))) => {
                ui.label(trf("No view of this part: {why}", &[("why", why)]));
            }
            None => {
                ui.label(tr("The bundle has no parts."));
            }
        });
    }

    fn tools(&self) -> Vec<Tool> {
        vec![Tool {
            id: "tab",
            icon: icons::OPEN,
            label: "Open in a tab",
        }]
    }

    fn context(&self) -> Vec<Tool> {
        self.tools()
    }

    /// The part chosen, for Open with.
    fn offer(&self) -> Option<Offer> {
        let part = self.parts.get(self.selected)?;
        Some(Offer {
            name: part.name.clone(),
            mime: Some(part.mime.clone()),
        })
    }

    fn offered(&mut self) -> Result<BlockFile, String> {
        self.parts
            .get(self.selected)
            .map(|p| p.bytes.clone())
            .ok_or_else(|| tr("The bundle has no parts.").to_owned())
    }

    fn act(&mut self, id: &str, host: &mut Host) {
        if id == "tab" {
            host.requests.extend(self.in_a_tab(self.selected));
        }
    }

    fn search(&mut self) -> Option<&mut Search> {
        match &mut self.inner {
            Some((_, Ok(inner))) => inner.search(),
            _ => None,
        }
    }

    fn status(&self) -> String {
        let Some(part) = self.parts.get(self.selected) else {
            return tr("The bundle has no parts.").to_owned();
        };
        trf(
            "Part {n} of {count}: {name}, {mime}, at {offset}",
            &[
                ("n", &(self.selected + 1)),
                ("count", &self.parts.len()),
                ("name", &part.name),
                ("mime", &part.mime),
                ("offset", &part.bytes.offset()),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_bundle_is_its_parts() {
        let bytes = crate::app::FILES
            .get("samples/demo.wwbundle")
            .unwrap()
            .bytes;
        let file = BlockFile::from_static(bytes);
        let parts = parse(&file).unwrap();
        let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "hello.hello",
                "elephants.mandel",
                "readme.txt",
                "gradient.bin"
            ]
        );
        assert_eq!(
            parts[0].bytes.to_vec(64).unwrap(),
            b"Hello from a part of a bundle\n"
        );
        assert_eq!(parts[3].bytes.len(), 4096);
        assert_eq!(parts[3].bytes.read(255, 2).unwrap(), [255, 0]);
        assert_eq!(BundlePlugin.sniff(bytes), Some(MIME));
        let broken = BlockFile::from_bytes(b"WWBUNDLE 1\na b 99\n\nshort".to_vec());
        assert_eq!(
            parse(&broken).err().unwrap(),
            "the part a goes past the end"
        );
        let none = BlockFile::from_bytes(b"other".to_vec());
        assert!(parse(&none).is_err());
    }
}
