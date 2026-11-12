//! The tests of the demo: its translations, its embedded guide and the links between its pages, a
//! file of the set opened by the plugin of its type, the parts of a bundle inside its view and in
//! tabs of their own, Open with, a note saved and its draft, the widgets, the question, the count
//! in the background, the window drawn.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use wwrt::egui;
use wwrt::egui_kittest::Harness;
use wwrt::host::{HostAct, HostDoc, PluginApp};
use wwrt::plugin::Plugins;
use wwrt::testing;
use wwrt::ww::{App, Document, Settings, ShellAct, ShellDoc, Window};

use crate::app::{Act, Demo, Doc, Extra, FILES, GUIDE, own_plugins};
use crate::widgets::WidgetAct;

/// The plugins of the tests: the template as a library and the two of the program.
fn plugins() -> Arc<Plugins> {
    let mut plugins = Plugins::default();
    plugins.add(
        PathBuf::from("ww_hello.dll"),
        Box::new(ww_hello::HelloPlugin),
    );
    own_plugins(&mut plugins);
    Arc::new(plugins)
}

/// Settings in English.
fn english() -> Settings<Extra> {
    Settings {
        language: "en".to_owned(),
        ..Settings::default()
    }
}

fn window() -> Window<Demo> {
    let mut w = Window::new(Demo::new(plugins(), Vec::new()), english());
    App::restored(&mut w.app, &mut w.shell);
    w
}

/// A folder of its own for a test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ww-demo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The page of a set of the tab in front, by its path.
fn page(w: &Window<Demo>) -> Option<String> {
    match w.shell.active_doc().and_then(Document::shell_doc) {
        Some(ShellDoc::Page(p)) => Some(p.path.clone()),
        _ => None,
    }
}

/// The name of the tab in front and the status of its view, when it is a view of a plugin.
fn front(w: &Window<Demo>) -> Option<(String, String)> {
    match w.shell.active_doc() {
        Some(Doc::Host(HostDoc::File(d))) => Some((d.name.clone(), d.view.status())),
        _ => None,
    }
}

#[test]
fn every_text_of_the_window_is_translated() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut wanted = testing::plugin_texts(&root.join("src"));
    wanted.extend(testing::texts(Path::new(wwrt::SOURCES)));
    wanted.extend(testing::texts(Path::new(wwrt::ww::SOURCES)));
    wanted.extend(testing::texts(Path::new(wwrt::wwmcp::SOURCES)));
    // The texts the window translates without a literal of its own: the hint of New, the hints of
    // the search of the tabs, the sections of the settings, the colour of the themes.
    wanted.extend(
        [
            "New note (Ctrl+N)",
            "Search the text (Ctrl+F)",
            "Search the page (Ctrl+F)",
            "Search the note (Ctrl+F, F3 the next)",
            "Search the items (Ctrl+F)",
            "Demo",
            "Plugins",
            "Star of the demo",
        ]
        .map(String::from),
    );
    let languages = FILES.translations();
    assert_eq!(
        languages.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        ["uk", "ru"]
    );
    let missing = testing::missing(&languages, &wanted);
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn the_guide_opens_and_its_links_lead_to_the_other_pages() {
    let mut w = window();
    assert_eq!(page(&w).as_deref(), Some(GUIDE), "a start without a file");
    w.apply(vec![Act::Shell(ShellAct::OpenLink("window.md".to_owned()))]);
    assert_eq!(page(&w).as_deref(), Some("docs/window.md"));
    w.apply(vec![Act::Shell(ShellAct::OpenLink(
        "guide.md#the-demo-of-w".to_owned(),
    ))]);
    assert_eq!(page(&w).as_deref(), Some(GUIDE), "the open tab of the page");
    assert_eq!(w.shell.docs.len(), 2);
    // The guide in Ukrainian: its translation stands for it, a page without one stays English.
    let uk = FILES.read(GUIDE, "uk").unwrap();
    assert!(String::from_utf8_lossy(&uk).starts_with("# "));
    assert_ne!(&uk[..], FILES.get(GUIDE).unwrap().bytes);
    assert_eq!(
        FILES.read("docs/files.md", "uk").unwrap().as_ref(),
        FILES.get("docs/files.md").unwrap().bytes
    );
}

#[test]
fn an_embedded_bundle_opens_with_its_parts_inside() {
    let mut w = window();
    w.apply(vec![Act::Shell(ShellAct::OpenEmbedded(
        FILES,
        "samples/demo.wwbundle".to_owned(),
    ))]);
    let (name, status) = front(&w).expect("the bundle by the plugin of bundles");
    assert_eq!(name, "demo.wwbundle");
    assert!(status.starts_with("Part 1 of 4: hello.hello"), "{status}");
    // Drawn: the first part, a hello, inside the view of the bundle.
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_eframe(|_| w);
    h.run_steps(3);
    let w = h.state_mut();
    // Open in a tab: the part by the plugin of its type, a part of the bundle without a copy.
    w.apply(vec![Act::Host(HostAct::Tool("tab"))]);
    let pending = std::mem::take(&mut w.shell.pending);
    w.apply(pending);
    let (name, status) = front(w).expect("the part in a tab of its own");
    assert_eq!(name, "hello.hello");
    assert!(status.contains("points"), "{status}");
}

#[test]
fn the_text_of_a_hello_opens_with_the_plugin_of_texts() {
    let dir = folder("hello");
    std::fs::write(dir.join("a.hello"), "Hello there\n").unwrap();
    let mut w = window();
    w.apply(vec![Act::Shell(ShellAct::OpenPath(dir.join("a.hello")))]);
    assert_eq!(front(&w).unwrap().0, "a.hello");
    let choices = w.app.plugins().choices_for_bytes(Some("text/plain"));
    let texts = choices
        .iter()
        .find(|c| c.found && w.app.plugins().loaded[c.plugin].plugin.name() == "Texts")
        .cloned()
        .expect("the plugin of texts takes the text of a hello");
    w.apply(vec![Act::Host(HostAct::OpenOffered(texts))]);
    let pending = std::mem::take(&mut w.shell.pending);
    w.apply(pending);
    let (name, status) = front(&w).unwrap();
    assert_eq!((name.as_str(), status.as_str()), ("Hello there", "1 lines"));
    // A file of another name whose first line is a hello opens by the template.
    std::fs::write(dir.join("greeting.data"), "Hello by its bytes\n").unwrap();
    w.apply(vec![Act::Shell(ShellAct::OpenPath(
        dir.join("greeting.data"),
    ))]);
    let (_, status) = front(&w).unwrap();
    assert!(status.contains("points"), "{status}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_note_is_saved_and_a_page_of_the_disk_opens() {
    let dir = folder("note");
    std::fs::write(dir.join("a.txt"), "first line\r\n").unwrap();
    std::fs::write(dir.join("b.md"), "# B\n\n[a](a.txt)\n").unwrap();
    let mut w = window();
    w.apply(vec![Act::Shell(ShellAct::OpenPath(dir.join("a.txt")))]);
    w.apply(vec![Act::Greet]);
    assert!(w.shell.active_doc().unwrap().dirty());
    w.apply(vec![Act::Shell(ShellAct::Save)]);
    let saved = std::fs::read_to_string(dir.join("a.txt")).unwrap();
    assert_eq!(
        saved, "first line\r\nHello from the settings\r\n",
        "its line ends kept"
    );
    assert!(!w.shell.active_doc().unwrap().dirty());
    w.apply(vec![Act::Shell(ShellAct::OpenPath(dir.join("b.md")))]);
    assert!(matches!(w.shell.active_doc(), Some(Doc::Page(_))));
    w.apply(vec![Act::Shell(ShellAct::OpenLink("a.txt".to_owned()))]);
    assert!(
        matches!(w.shell.active_doc(), Some(Doc::Note(_))),
        "the open note"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_widgets_the_question_and_the_count() {
    let mut w = window();
    w.apply(vec![Act::Widgets]);
    w.apply(vec![
        Act::Widget(WidgetAct::Choose(1)),
        Act::Widget(WidgetAct::Check),
    ]);
    let nav = App::nav(&w.app, &w.shell).unwrap();
    assert!(nav.history.as_ref().unwrap().0, "Back after a choice");
    assert!(App::messages(&w.app, &w.shell).is_some());
    w.apply(vec![Act::Ask]);
    assert!(w.shell.question.is_some());
    w.apply(vec![Act::Shell(ShellAct::AnswerAlt)]);
    assert_eq!(w.shell.status, "The answer: Later");
    w.shell.settings.extra.count_to = 2;
    w.apply(vec![Act::StartJob]);
    assert_eq!(App::jobs(&w.app, &w.shell).len(), 1);
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .build_eframe(|_| w);
    let started = std::time::Instant::now();
    while !h.state().shell.status.starts_with("The count stopped") {
        assert!(started.elapsed().as_secs() < 20, "the count ends");
        std::thread::sleep(std::time::Duration::from_millis(50));
        h.run_steps(1);
    }
    assert_eq!(h.state().shell.status, "The count stopped at 2 of 2");
}

#[test]
fn the_page_of_the_plugins_and_the_settings_draw() {
    let mut w = window();
    w.apply(vec![Act::Host(HostAct::Plugins)]);
    assert!(matches!(
        w.shell.active_doc(),
        Some(Doc::Host(HostDoc::Plugins))
    ));
    w.apply(vec![Act::Shell(ShellAct::OpenSection(2))]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_eframe(|_| w);
    h.run_steps(3);
    let w = h.state_mut();
    w.apply(vec![Act::Shell(ShellAct::Activate(1))]);
    h.run_steps(2);
}

/// The tools of the window and of the program through the server: a client of another thread
/// opens a note, lists the tabs and closes it, answered at the frames of the window; the tools
/// of the plugins are listed with their sources, and the tab of the server opens.
#[test]
fn the_tools_of_the_window_answer_a_client() {
    use wwrt::serde_json::json;
    use wwrt::wwmcp::{Caller, Config, Mode};

    let mut w = window();
    let server = w.app.mcp().unwrap().panel.server.clone();
    let sources: Vec<String> = server.tools().into_iter().map(|l| l.source).collect();
    assert!(sources.starts_with(&["window".to_owned()]), "{sources:?}");
    let names: Vec<String> = server.tools().into_iter().map(|l| l.tool.name).collect();
    for name in ["list_files", "open_file", "new_note", "hello_greet"] {
        assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
    }
    server.set_config(Config {
        mode: Mode::Edit,
        ..Config::default()
    });
    let client = server.clone();
    let thread = std::thread::spawn(move || {
        let note = client.call(
            "new_note",
            &json!({ "text": "from an agent" }),
            &Caller::Client,
        )?;
        let tabs = client.call("list_files", &json!({}), &Caller::Client)?;
        let closed = client.call(
            "close_file",
            &json!({ "tab": note["tab"] }),
            &Caller::Client,
        )?;
        Ok::<_, wwrt::wwmcp::ToolError>((note, tabs, closed))
    });
    let ctx = egui::Context::default();
    let bridge = w.app.mcp().unwrap().bridge.clone();
    for _ in 0..2000 {
        wwrt::wwmcp::window::answer(
            &mut w.app,
            &mut w.shell,
            &bridge,
            &ctx,
            &mut |app, shell, tool, args| app.program_tool(shell, tool, args),
        );
        if thread.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let (note, tabs, closed) = thread.join().unwrap().unwrap();
    assert_eq!(closed["tab"], note["tab"]);
    let listed = tabs.as_array().unwrap();
    assert!(listed.iter().any(|t| t["tab"] == note["tab"]), "{tabs}");
    assert!(!w.shell.docs.iter().any(|d| matches!(d, Doc::Note(_))));
    // The mode read refuses the tool that opens a tab.
    server.set_config(Config::default());
    let refused = server
        .call("new_note", &json!({}), &Caller::Client)
        .unwrap_err();
    assert_eq!(refused.kind, wwrt::wwmcp::Kind::NotAllowed);
    w.apply(vec![Act::Host(HostAct::OpenMcp)]);
    assert!(matches!(
        w.shell.active_doc(),
        Some(Doc::Host(HostDoc::Mcp(_)))
    ));
}

#[test]
fn settings_that_did_not_read_load_no_dll() {
    use wwrt::host::PluginSettings;

    let dir = folder("unread");
    std::fs::write(dir.join("other.dll"), b"no plugin").unwrap();
    // Settings that read (the defaults trust every DLL): the DLL of the folder is tried.
    let read = crate::app::plugins_at_start(&dir, Some(&PluginSettings::default()));
    assert_eq!((read.loaded.len(), read.failed.len()), (2, 1));
    // Settings that did not read: the folder is not looked at, the program's own plugins stay.
    let unread = crate::app::plugins_at_start(&dir, None);
    assert_eq!((unread.loaded.len(), unread.failed.len()), (2, 0));
    let _ = std::fs::remove_dir_all(&dir);
}
