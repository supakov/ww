# WW (W#): a window with tabs for programs that analyse and manage a system

WW is the framework of programs with document tabs: the crates `ww-base` and `ww` of this
repository, which the programs take as a git dependency of its branch `main`. A program on WW is
one executable with its data folders next to it, a window with document tabs, a menu, a
command bar of the active tab, a navigation row where a tab has one, a message bar, a status bar, a
settings tab, themes with their fonts and an editor, translations, drafts of unsaved changes and the
tabs of the last session. The program brings its documents, its acts and what it draws inside a tab;
the shell does the rest. This page is for the people who write such a program.

## 1. The crates

| Crate | What it holds |
|---|---|
| `ww-base` | Pure Rust, no window: `jsonfile` (strict JSON documents with the trait `Document`, atomic saves), `paths` (a file inside the program folder kept relative to it), `clock`. |
| `ww` | The window: `shell` (the trait `App`, `Shell`, `Window`, `ShellAct`, `ShellDoc`), `run` (`run::<A>`, `smoke_window::<A>`), `web` (`start::<A>` in a browser, wasm32 only), `doc` (the trait `Document`, `Search`, `Draft`, `Scrolled`), `settings` (`Settings<X>` with the fields of the program flattened in), `session` (drafts, the log), `store` (the files of the window: next to the executable natively, the storage of the page and downloads in a browser), `style` (the palette, the fonts, the painted buttons and cards), `themes` and `themeview` (theme files, the editor), `i18n` (`tr`, `trf`, the dictionaries), `tree` and `list` (the painted tree and list with the keyboard), `widgets` (a modal window, a side panel, a dialog in a folder), `mdview` (Markdown pages), `renderer` (OpenGL and DirectX 12), `commands` (the items of a command bar), `keys` (the keys of the window), `icons`, `dialogs`, `folders`, `embedded` (the files embedded in the executable or a plugin,
`embed!`, docs/embedded.md), `textdoc` (a text file in the editor of egui: the search, the
cursor, the draft, the line ends kept, `read_text`). `ww` re-exports `eframe` (with `egui`) and `rfd`, so a program needs no other crate for its window. |

A program depends on `ww` with `default-features = false` and chooses its renderers with the
features `glow` (OpenGL) and `wgpu` (DirectX 12), both by default. Built for `wasm32-unknown-unknown`
with `glow` alone, the same program runs in a browser (section 8).

## 2. A program in five parts

1. **The documents.** One enum of every kind of tab, boxed where a kind is large, with a variant
   that holds `ShellDoc` (the settings tab and the editor of a theme, documents of the window
   itself). The enum implements `ww::Document`: the title, whether it is dirty, its file, its
   search, its icon, the hint of its search field, whether it has a command bar and a navigation
   row, how it scrolls to its selection, its draft (`draft_key`, `draft_due`) and `shell_doc` for
   the variant of the window.
2. **The acts.** One enum of what a frame requests: a variant `Shell(ShellAct<Self>)` for the acts
   of the window (the tabs, open, save, the settings, the themes, a question, a message, a link)
   and the program's own. `From<ShellAct<Self>> for Act` lets the shell push its acts.
3. **The settings.** A struct of the program's fields with `#[serde(flatten)]` behind
   `ww::Settings<X>`: `impl ww::Extra for X { const FORMAT: &str = "<program>-settings"; }`.
   `Settings<X>` holds the language, the theme, the renderer, the recent files, the open tabs, the
   searches; the program's fields sit beside them in the one `settings.json`.
4. **The program.** `impl ww::App for P`: the types `Doc`, `Act`, `Extra`; `NAME` (the title of the
   window, the name of the log), `BUILD`, `NEW_HINT`; `shell_doc` and `shell_act` (the two
   conversions); `started` (the first tab, at every construction, the tests too), `load` (the
   files of the program at a start), `restored` (after the tabs of the last session and the drafts
   are back), `closing` (before the files of the tabs are remembered, for what the program keeps
   beyond them); `new_document`, `open_path`, `open_dialog` (`open_filters` and `open_bytes` in a
   browser), `save`, `restore_draft`; `draw` (the
   active tab), `commands`, `menu`, `menu_popup`, `nav`, `messages` and `messages_body`,
   `status_left`, `status_right`, `busy`; `settings_sections`, `settings_general`,
   `settings_section`, `settings_opened`; `shortcuts`, `apply`, `dialogs`, `after_frame`;
   `languages` and `themes` (what the executable embeds), `system_language`. Most have a default
   that does nothing.
5. **The start.** `ww::run::<P>(renderer, |settings| P::new(..))` reads the settings, tries the
   renderers, installs the fonts, builds the window, restores the session and runs the event loop;
   `ww::smoke_window::<P>(renderer)` draws two frames and closes, for CI.

## 3. The shell

`Shell<P>` is the state of the window the program sees in every call: `docs` and `active` (the
tabs), `settings` and `settings_dirty`, `status` and `shown_file` (the message of the status bar
and the file its button opens), `question`, `messages_open`, `themes`, `drafts_dir`, `logs_dir`,
`graphics` (what the window draws with), `pending` (acts a view requests outside the list of a
frame). Its methods do what every program needs: `open_tab`, `open_file` (a file in a tab, once),
`show_or_open` and `focus_tab`, `close_tab`, `move_tab`, `active_doc`, `active_doc_mut`,
`active_search`, `saved`, `opened`, `save_failed`, `open_failed` (the messages of the status bar),
`remember_recent`, `ask` (a question before an act), `close_question` (Save, Don't save or Cancel
before a tab with changes closes), `linked_file` (the file a link of the active page names,
relative to the page), `about_lines` (the lines every About has), `open_settings`,
`effective_language`, `apply_language`, `log`, `open_page` and `active_page` (a page of a set of
embedded files, `ShellDoc::Page`).

`Window<P>` holds the shell and the program apart, so a method of the program takes `&mut self`
and `shell: &mut Shell<P>` without a double borrow. `Window::new(program, settings)` is what a
test builds; `window.apply(acts)` runs acts as a frame would; `window.shell.docs` and
`window.app` are the state to assert on. `egui_kittest` draws a `Window` without a screen.

`ShellAct<P>` names the acts of the window: `Activate`, `MoveTab`, `CloseTab`, `New`, `Open`,
`OpenPath`, `OpenBytes` (a file by its name and bytes, from a browser), `Save`, `SaveAs`,
`ForgetRecent`, `OpenSettings`, `OpenSection`, `EditTheme`,
`OpenThemeFile`, `UseTheme`, `ToggleMessages`, `SearchCleared`, `Revealed`, `Follow`, `Message`,
`ShowInFolder`, `OpenLink`, `OpenEmbedded` (a file of a set of embedded files: a page in a tab of
the window, `App::open_embedded`), `OpenWeb`, `About`, `SaveAndClose`, `Exit`, `Ask`, `Answer`,
`AnswerAlt`. A question (`ww::Question`) has a title, a text, the yes and its act, and with
`Question::or` a second answer beside it. The keys of the window (`ww::keys`): Ctrl+N,
Ctrl+O, Ctrl+S, Ctrl+Shift+S, Ctrl+W, Ctrl+comma, Ctrl+F, Ctrl+Tab and Ctrl+Shift+Tab,
Ctrl+Shift+PgUp and PgDn; the program adds its own in `shortcuts`.

## 4. What the program draws

- The command bar is data: `commands` gives `Vec<Item<Act>>` made with `cmd`, `cmd_if`, `when`,
  `keyed` and `menu` of `ww::commands`; a menu button names its number, and `menu_popup` draws
  that menu. Below 1100 px the bar moves into the navigation row as icons.
- The menu button of the tab row calls `menu`; `ww::shell::menu_item` draws an item with its keys.
- The navigation row appears for a document whose `has_nav` is true: `nav` gives the history
  (back and forward with their acts), the breadcrumb (labels with their acts) and the star.
- The message bar appears when `messages` gives a head (an error or a warning, a summary, the
  act that clears it); `messages_body` draws the list.
- The status bar shows `status_left` (texts), then `status_right` (anything), the button of the
  folder of a saved file, the message, a spinner while `busy`.
- The settings tab has the General section of the window (language, theme and its fonts: `App::edits_themes` offers the editor of a theme, `App::chooses_fonts` the fonts alone, the
  size among `style::SIZE_STEPS`, 10 to 36 points, the chrome growing with it through
  `style::at_least`; the
  window: the program's lines through `settings_general`, the renderer (not in a browser, which
  draws with WebGL alone), the tabs of the last session) and the sections of the program
  (`settings_sections`, `settings_section`).
- A program adds colours to the themes with `ww::style::install_colors` (`ExtraColor`: the name in a
  theme file, the label of the theme editor, the light and the dark value); `style::extra(ui, name)`
  gives the one of the theme shown, a theme file overrides it by name, and the editor lists it after
  the palette. The palette has `code_bg` for the code of a page.
- `Settings::wrap_lines` (General, Wrap lines to the width of the view) is what a text or a page
  follows: `ww::mdview::md_view(ui, doc, wrap, page)` folds the lines or scrolls sideways, and
  makes the page as wide as `Settings::page_width` (`PageWidth`: the view, or at most so many pixels
  or characters, Width of pages in General): its text wraps at that width, its tables fill it with their cells
  wrapped.
- About is a window over the tabs (`ShellAct::About`): the name of the program and the lines of
  `about`, by default the version, the settings file and the renderer.
- Every link of a page goes through the window: a heading scrolls the page, a `.md` or `.txt`
  file or a file without an extension goes to `open_link` (the program opens it, relative to the
  page through `Shell::linked_file`), a web link or a file of another kind is asked first and
  goes to the browser. `MdDoc::plain` shows a text file as it is. A page scrolls with the arrows,
  the page keys, Home and End while no text field has the focus, and keeps its place and its
  selected text when another tab comes in front.
- A theme names the font of the icons (Segoe UI Emoji by default, or Segoe UI Symbol). With Segoe
  UI Emoji, `style::paint_icon` draws an icon as a picture in the colours of the font
  (`ww::coloricons`, DirectWrite); the command bar and the tabs use it, any other text keeps the
  glyph in one colour, since egui draws glyphs in one colour.
- `Settings::window` is where the window was at the last close; `run` opens it there and the first
  frame fits it to its monitor.
- The look comes from `ww::style`: `palette(ui)` for the colours, `font`, `strong`, `muted`,
  `mono`, `size`, `row_height` for the text, `card`, `block`, `header`, `section_title`, `chip`,
  `icon_button`, `tool_button`, `selectable_text`, `copy_label`. `ww::tree::show` and
  `ww::list::show` paint a tree and a list with the keyboard.

### The work in the background, the menus, the tabs

- `App::jobs` lists the work that runs in the background as `ww::Job`s (a title, a detail, when it
  started, the act of Stop); the shell shows them in a panel over the lower right corner, folded to a
  spinner on request, and keeps the spinner of the status bar while any runs.
- A menu item is `shell::menu_item` (an icon, the text, the keys, the act) or `shell::menu_entry`
  (true when clicked), a menu inside a menu `shell::sub_menu`; every item keeps the place of an icon,
  so the texts line up. `commands::toggle` is a switch of the command bar, drawn pressed when on.
- The tab row scrolls sideways when the tabs are wider than the window (the wheel, the active tab
  brought into view) and a button at its right end lists every tab.

## 5. Next to the executable

`<exe>.settings.json` (the settings, named after the executable so that two copies with different
names keep their own; `<program>.settings.json` and a `settings.json` of an earlier version are read
and kept when they exist; `Settings::load` finds the file and `Shell::settings_path` names it),
`drafts/` (a draft two seconds after a change, `{"path", "document"}` with the
document the program gives, read back through `restore_draft`), `logs/<name>-<date>.log` (every
new message of the status bar, through `Shell::log` too), `themes/` (`<id>.wwtheme` files; the
embedded ones are Light and Dark, and `App::themes` adds more), `lang/<code>/ui.json`
(translations of every `tr` and `trf` text of the program and of WW; `App::languages` embeds
some, a file adds or overrides one; English is the source and the fallback).

The settings, the drafts and the themes go through `ww::store` (`exists`, `read`, `write`,
`remove`, `list`, `log_line`): natively the files above, in a browser the `localStorage` of the
page under the key `ww/<path>` and the console for the log. A program that keeps a file of its own
next to the executable uses the store too when it wants it in a browser.

## 8. In a browser

Built for `wasm32-unknown-unknown` with the feature `glow` alone, a program starts with
`ww::web::start::<P>(canvas_id, make).await` in a task of the page (`wasm_bindgen_futures::spawn_local`
from `main`): eframe's `WebRunner` draws in the canvas of that id, the log of eframe goes to the
console, and a failure of the start replaces the text of the element `loading_text` of the page.
The page loads the module through the glue `wasm-bindgen --target web` writes and is served
over http. Files come by their bytes: `ShellAct::Open`
runs `rfd::AsyncFileDialog` with the kinds of `App::open_filters`, a drop on the page is read, and the
next frame hands each file to `App::open_bytes(shell, name, bytes)` as `ShellAct::OpenBytes`; a save is a
download through `ww::store::download(name, bytes, mime)` (the theme editor does it for Save as).
`App::open_dialog`, the synchronous dialogs of rfd, `widgets::in_folder`, the title bar colours and
`ww::run` are native only. The keys Ctrl+N, Ctrl+W and Ctrl+Tab belong to the browser.

The files of a program and of the libraries it takes go through `ww_base::files` (`read`, `write`,
`exists`, `remove`, `list`; `jsonfile::read` and `write_atomic`, `ww::dialogs::write_file` use it):
natively the disk, in a browser the backend `ww::web::start` sets (`ww::store::install_files`): the
storage of the page, and a download for a path under `ww_base::files::DOWNLOAD` (`download_path`),
so code that saves a document by a path a dialog gave saves it in a browser too when its dialog
gives such a path. Before the window starts, `ww::store::preload` reads `files.json` next to the
page (a list of relative paths) and every file it names over http; the store, the file layer and
the languages of `lang/` read them before the storage of the page, until the program writes a
file of the same path: what it writes goes into the storage and is read from then on, in that
visit of the page. The storage keeps texts: bytes that are no UTF-8 are refused by the file layer,
never stored changed. A page opened from `file://` reads none, and a browser lists no folder of a
server, so the list is that file.

Natively `ww_base::files::write` (and `write_disk`, which `ww::store::write` and the
configuration of the MCP server use) writes through a temporary neighbour of a name of its own,
`<name>.<process>-<number>.tmp`, made anew, synced to the disk and renamed over the target; it is
removed when the write fails, and no file of the user (`report.tmp` next to `report.json`) is
taken for it.

## 6. Texts and translations

Every text of the interface is `tr("English")` or `trf("English {name}", &[("name", &value)])`.
A dictionary is a JSON object of English keys and translated values with `_language` for the
name of the language. The test of the translations of a program reads its own literals and the
ones of `ww` (its sources at `ww::SOURCES`, the checkout of git) and fails when an embedded
language lacks one; every program keeps its own files complete (the demo embeds
`lang/uk/ui.json` and `lang/ru/ui.json` with every text of the demo, of W# and of wwrt). A
program may take its languages from a set of embedded files (`Embedded::translations`).

## 7. The demo, the example

`examples/ww-demo` shows every part once: `app.rs` (`Demo`, `Doc { Host, Note, Page, Widgets,
Shell }`, `Act`, `Extra` with the settings of the plugins flattened in, `impl ww::App` and
`impl wwrt::host::PluginApp`, the files it embeds), `widgets.rs` (the tree with a context menu on
its rows, the list, the navigation row, the message bar, the painted widgets of the style),
`bundle.rs` and `texts.rs` (two plugins the program makes itself), `main.rs` (the command line,
`ww::run`) and `tests.rs` (the window built with `Window::new` and drawn with egui_kittest).
`build-all.cmd` packs it with its plugins as `dist/ww-demo-<version>.zip`.
