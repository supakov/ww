//! The tab of the widgets of W#: a painted tree of folders with their boxes and a context menu on
//! its rows (`ww::tree::show_with_menu`), the painted list of the folder chosen with its columns
//! (`ww::list::show`), the navigation row (Back and Forward through the folders chosen, the
//! breadcrumb, the star of a folder), the message bar of a check, and the painted widgets of the
//! style (a card, chips, a selectable text, a copy label, a colour of the program).

use wwrt::egui::{self, RichText};
use wwrt::ww::i18n::{tr, trf};
use wwrt::ww::list::{self, Cell, Column};
use wwrt::ww::shell::menu_entry;
use wwrt::ww::tree::{self, Check, Row, Scroll};
use wwrt::ww::{Document, Search, icons, style};

/// The colour the program adds to the themes (`style::install_colors`): the star.
pub const STAR: &str = "demo_star";

/// An item of a folder: its name and how many there are.
#[derive(Clone)]
pub struct Item {
    pub name: String,
    pub count: u32,
}

/// A folder of the tree: its items, whether it is checked, starred and open.
#[derive(Clone)]
pub struct Folder {
    pub name: String,
    pub items: Vec<Item>,
    pub checked: bool,
    pub starred: bool,
    pub open: bool,
}

/// What the tab asks of the program after a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum WidgetAct {
    /// A folder chosen (in the tree, the breadcrumb or the history).
    Choose(usize),
    Back,
    Forward,
    Star,
    /// One more of an item of the folder chosen.
    AddOne(usize),
    /// A folder of the tree, its name in capitals: an item of the context menu of its row.
    Shout(usize),
    Check,
    ClearMessages,
    /// The switch of the bar: the counts in the list.
    Counts,
}

/// The tab of the widgets.
pub struct Widgets {
    pub folders: Vec<Folder>,
    pub chosen: usize,
    /// The folders chosen before and after the one shown, for Back and Forward.
    pub back: Vec<usize>,
    pub forward: Vec<usize>,
    /// The messages of the last check: an error or a warning each.
    pub messages: Vec<(bool, String)>,
    pub counts: bool,
    pub search: Search,
    pub item: Option<usize>,
}

impl Default for Widgets {
    fn default() -> Self {
        let item = |name: &str, count| Item {
            name: name.to_owned(),
            count,
        };
        Widgets {
            folders: vec![
                Folder {
                    name: String::from("Fruit"),
                    items: vec![item("Apple", 3), item("Pear", 0), item("Plum", 12)],
                    checked: true,
                    starred: false,
                    open: true,
                },
                Folder {
                    name: String::from("Tools"),
                    items: vec![item("Hammer", 1), item("Saw", 2)],
                    checked: false,
                    starred: true,
                    open: true,
                },
                Folder {
                    name: String::from("Books"),
                    items: vec![item("Atlas", 0)],
                    checked: false,
                    starred: false,
                    open: false,
                },
            ],
            chosen: 0,
            back: Vec::new(),
            forward: Vec::new(),
            messages: Vec::new(),
            counts: true,
            search: Search::default(),
            item: None,
        }
    }
}

impl Document for Widgets {
    fn title(&self) -> String {
        tr("Widgets").to_owned()
    }

    fn search(&mut self) -> Option<&mut Search> {
        Some(&mut self.search)
    }

    fn icon(&self) -> &'static str {
        icons::SELECT_ALL
    }

    fn search_hint(&self) -> &'static str {
        "Search the items (Ctrl+F)"
    }

    fn has_nav(&self) -> bool {
        true
    }
}

impl Widgets {
    /// Runs an act of the tab; a line for the status bar when it has one.
    pub fn apply(&mut self, act: WidgetAct) -> Option<String> {
        match act {
            WidgetAct::Choose(i) if i < self.folders.len() && i != self.chosen => {
                self.back.push(self.chosen);
                self.forward.clear();
                self.chosen = i;
                self.item = None;
            }
            WidgetAct::Choose(_) => {}
            WidgetAct::Back => {
                if let Some(i) = self.back.pop() {
                    self.forward.push(self.chosen);
                    self.chosen = i;
                }
            }
            WidgetAct::Forward => {
                if let Some(i) = self.forward.pop() {
                    self.back.push(self.chosen);
                    self.chosen = i;
                }
            }
            WidgetAct::Star => {
                let folder = &mut self.folders[self.chosen];
                folder.starred = !folder.starred;
            }
            WidgetAct::AddOne(i) => {
                let item = self.folders[self.chosen].items.get_mut(i)?;
                item.count += 1;
                return Some(trf(
                    "{name}: {n}",
                    &[("name", &item.name), ("n", &item.count)],
                ));
            }
            WidgetAct::Shout(i) => {
                let folder = self.folders.get_mut(i)?;
                folder.name = folder.name.to_uppercase();
            }
            WidgetAct::Check => {
                self.messages.clear();
                for f in &self.folders {
                    for item in &f.items {
                        if item.count == 0 {
                            let text = trf(
                                "{folder}: no {item} left",
                                &[("folder", &f.name), ("item", &item.name)],
                            );
                            self.messages.push((f.checked, text));
                        }
                    }
                }
                if self.messages.is_empty() {
                    return Some(tr("Every item is there").to_owned());
                }
            }
            WidgetAct::ClearMessages => self.messages.clear(),
            WidgetAct::Counts => self.counts = !self.counts,
        }
        None
    }

    /// The breadcrumb: the tab, then the folder chosen.
    pub fn crumbs(&self) -> Vec<(String, Option<WidgetAct>)> {
        vec![
            (tr("Widgets").to_owned(), None),
            (
                self.folders[self.chosen].name.clone(),
                Some(WidgetAct::Choose(self.chosen)),
            ),
        ]
    }

    /// The summary of the messages for the message bar.
    pub fn summary(&self) -> Option<(bool, String)> {
        if self.messages.is_empty() {
            return None;
        }
        let errors = self.messages.iter().filter(|(e, _)| *e).count();
        Some((
            errors > 0,
            trf(
                "{n} items are missing, {errors} of them in a folder checked",
                &[("n", &self.messages.len()), ("errors", &errors)],
            ),
        ))
    }

    /// The messages, one a line.
    pub fn messages_body(&self, ui: &mut egui::Ui) {
        for (error, text) in &self.messages {
            let color = if *error {
                ui.visuals().error_fg_color
            } else {
                ui.visuals().warn_fg_color
            };
            ui.colored_label(color, text);
        }
    }

    /// The rows of the tree: the folders and, under an open one, its items.
    fn rows(&self) -> Vec<Row<(usize, Option<usize>)>> {
        let mut rows = Vec::new();
        for (i, f) in self.folders.iter().enumerate() {
            rows.push(Row {
                key: (i, None),
                depth: 0,
                title: f.name.clone(),
                right: trf("{n} items", &[("n", &f.items.len())]),
                check: if f.checked { Check::On } else { Check::Off },
                open: Some(f.open),
                muted: false,
                icon: Some(icons::FOLDER),
                switch: None,
            });
            if f.open {
                for (j, item) in f.items.iter().enumerate() {
                    rows.push(Row {
                        key: (i, Some(j)),
                        depth: 1,
                        title: item.name.clone(),
                        right: item.count.to_string(),
                        check: Check::None,
                        open: None,
                        muted: item.count == 0,
                        icon: None,
                        switch: None,
                    });
                }
            }
        }
        rows
    }

    /// Draws the tab; the acts that need the program go to `acts`.
    pub fn view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<WidgetAct>) {
        let rows = self.rows();
        let selected = rows.iter().position(|r| r.key == (self.chosen, self.item));
        egui::Panel::left(ui.id().with("widgets-tree"))
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| {
                let mut shout = None;
                let mut menu = |ui: &mut egui::Ui, key: &(usize, Option<usize>)| {
                    if key.1.is_none() && menu_entry(ui, icons::EDIT, tr("Capitals"), "", true) {
                        shout = Some(key.0);
                    }
                    if menu_entry(ui, icons::SHOW, tr("Choose"), "", true) {
                        acts.push(WidgetAct::Choose(key.0));
                    }
                };
                let out = tree::show_with_menu(
                    ui,
                    ui.id().with("tree"),
                    &rows,
                    selected,
                    Scroll::None,
                    &mut menu,
                );
                if let Some(i) = shout {
                    acts.push(WidgetAct::Shout(i));
                }
                for event in out.events {
                    match event {
                        tree::Event::Select((i, item)) => {
                            acts.push(WidgetAct::Choose(i));
                            self.item = item;
                        }
                        tree::Event::Toggle((i, None)) => {
                            self.folders[i].checked = !self.folders[i].checked;
                        }
                        tree::Event::Expand((i, None), open) => self.folders[i].open = open,
                        _ => {}
                    }
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let folder = &self.folders[self.chosen];
            style::card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    style::section_title(ui, &folder.name);
                    if folder.starred {
                        let star = style::extra(ui, STAR);
                        ui.label(RichText::new(icons::STAR).color(star).size(20.0));
                    }
                    style::chip(ui, &trf("{n} items", &[("n", &folder.items.len())]), true);
                    let state = if folder.checked {
                        tr("checked")
                    } else {
                        tr("not checked")
                    };
                    style::chip(ui, state, false);
                });
                style::copy_label(
                    ui,
                    RichText::new(trf("The folder {name}", &[("name", &folder.name)])),
                    true,
                );
            });
            ui.add_space(6.0);
            let words = self.search.words();
            let columns = [
                Column::new(tr("Item"), 200.0),
                Column::new(tr("Count"), 80.0),
                Column::new(tr("State"), 120.0),
            ];
            let shown: Vec<usize> = (0..folder.items.len())
                .filter(|j| wwrt::ww::matches_words(&folder.items[*j].name, &words))
                .collect();
            let rows: Vec<list::Row<usize>> = shown
                .iter()
                .map(|j| {
                    let item = &folder.items[*j];
                    let count = if self.counts {
                        Cell::new(item.count.to_string())
                    } else {
                        Cell::new("").muted()
                    };
                    let state = if item.count == 0 {
                        Cell::new(tr("none left")).warn()
                    } else {
                        Cell::new(tr("there")).chip(true)
                    };
                    list::Row {
                        key: *j,
                        cells: vec![Cell::new(&item.name).strong(), count, state],
                        button: Some((icons::ADD, tr("One more").to_owned())),
                    }
                })
                .collect();
            let selected = self
                .item
                .and_then(|j| shown.iter().position(|s| *s == j));
            let out = list::show(
                ui,
                ui.id().with("list"),
                tr("Items"),
                &columns,
                &rows,
                selected,
                Scroll::None,
            );
            for event in out.events {
                match event {
                    list::Event::Select(j) => self.item = Some(j),
                    list::Event::Button(j) | list::Event::Open(j) => acts.push(WidgetAct::AddOne(j)),
                    list::Event::Delete(_) => {}
                }
            }
            ui.add_space(6.0);
            style::selectable_text(
                ui,
                "widgets-hint",
                tr("The right button on a row of the tree opens its context menu; the button at the end of a row of the list adds one more."),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_the_star_and_the_check() {
        let mut w = Widgets::default();
        w.apply(WidgetAct::Choose(1));
        w.apply(WidgetAct::Choose(2));
        assert_eq!((w.chosen, w.back.clone()), (2, vec![0, 1]));
        w.apply(WidgetAct::Back);
        assert_eq!((w.chosen, w.forward.clone()), (1, vec![2]));
        w.apply(WidgetAct::Forward);
        assert_eq!(w.chosen, 2);
        w.apply(WidgetAct::Star);
        assert!(w.folders[2].starred);
        w.apply(WidgetAct::Check);
        assert_eq!(w.messages.len(), 2, "Pear and Atlas");
        let (error, _) = w.summary().unwrap();
        assert!(error, "Pear is in a folder checked");
        w.apply(WidgetAct::Choose(0));
        assert_eq!(w.apply(WidgetAct::AddOne(1)).as_deref(), Some("Pear: 1"));
        w.apply(WidgetAct::Shout(0));
        assert_eq!(w.folders[0].name, "FRUIT");
    }
}
