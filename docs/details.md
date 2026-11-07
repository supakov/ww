# The facts a change of W# must respect

Facts learned while building W# and its programs, out of `docs/spec/05-details.md` of WinWeaver
(digidult/winweaver), where they were found: the ones of the window of W#. The facts of wview and
its plugins are in `docs/details.md` of supakov/wview.

## The window and W#

- Text sizes: a theme has a size of `style::SIZES` (10 to 36), offered as `style::SIZE_STEPS` (by one to
  20, then 22, 24, 26, 28, 32, 36). The chrome scales with it through `style::at_least` (never below the
  size of the layout): the buttons of the command bar, the icon buttons, the tabs (`shell/tabs.rs`), the
  widths the navigation row keeps, the list of the settings sections. A new fixed size of the chrome goes
  through `at_least`. The screenshots `35-size-28`, `35-size-36` and `36-size-*-settings` show it.

- Renderers (`renderer.rs`, the features `glow` and `wgpu` of winweaver-ui and winweaver, both by default):
  OpenGL through glow takes half the memory of the full wgpu; DirectX 12 is wgpu with its `dx12` backend alone (no
  Vulkan, no OpenGL of wgpu) and draws without a driver of the graphics card on WARP (the CI runner shows
  `Microsoft Basic Render Driver (Dx12, Cpu)`). Automatic tries OpenGL, then DirectX 12; a choice of the
  settings (`Settings::renderer`) or of `--renderer` is tried first and the others follow. A renderer fails
  when `run_native` ends with an error before the app is created (egui_glow refuses OpenGL below 2.0,
  which is all Windows has without a driver); eframe keeps its event loop between the tries. The window
  logs what it draws with (`renderer::graphics_text`) and puts the failures in the message bar. The CI
  step Window smoke test runs `--smoke-window` and fails when no renderer starts; on the runner OpenGL fails
  and DirectX 12 draws, so it tests the fall back. Tests and screenshots draw with the wgpu of egui_kittest.

- The look comes from `style.rs`: the palette of light and dark (a theme overrides its tokens,
  `style::COLOR_NAMES`, also with the names of the older format) and the fonts of the theme. `install_fonts` (before the first frame) names the families `light`, `light-strong`,
  `light-mono` and the same for `dark`; `style::apply` loads the files of the families of the two themes
  into them (`style::FAMILIES`, `MONO_FAMILIES` in the fonts folder of Windows, egui's fonts as the
  fallback) and scales the text styles by the size of the theme. Painted widgets take their fonts and
  sizes from `style::font`, `strong_font`, `mono`, `size` and `row_height`, never `FontId::proportional`;
  without `install_fonts` (tests) the families of egui are used.

- Themes (`themes.rs`): a `.wwtheme` file is a name, a base, colours and fonts; the
  window keeps the list in `App::themes` (read at the start, when the settings open and after a save) and
  applies `wanted_theme` whenever it changes: the theme of an editor tab in front (a live preview), else
  the one of the settings, else Light and Dark as Windows shows its apps. A change of a font in the
  settings saves the file of the shown theme (`Theme::file_in`: an embedded theme gets
  `themes/<id>.wwtheme`).

- Lists (`list::show`): one stop of Tab like the tree; the arrows, Home, End and the page keys move
  the selection and set `follow`, Enter or a double click opens, Delete removes where the tab allows it,
  the widths of the columns change by dragging the lines between their titles (kept in the memory of egui).
  In the diff and bookmark tabs the list is the resizable side panel and the item on the right is the
  central panel: a side panel grows with what it holds, so a detail there moved the divider.

- Keyboard: a tree is one stop of Tab (`tree::show` makes a focusable area,
  the rows sense clicks only); a click on a row gives it the focus, so the keys go on from the clicked
  row: arrows, Home, End, Page Up and Down, Left and Right fold and go to the parent or child, Space
  toggles, Enter opens a group. A move with the keys sets `follow`, and the row comes into view at the next
  frame; the detail panel shows the new selection at once.

- Paths in the files of the program (`winweaver_core::paths`): a file inside
  the program folder is kept relative to it and starts with `.` (`.\profiles\Office.wwprofile`), any other
  file keeps its absolute path, and in memory every path is absolute. `Settings::load_from` and `save_to`
  (recent, open_tabs; relative to the folder of the settings), `Bookmarks::load` and `save` (the path of
  every bookmark but the registry ones), `Diff::load` and `save` (both sides) and the drafts (their path
  and `key_for_path`) convert at the file; `stored_text` changes only an absolute path, so a title of a
  preset or a key of the registry stays, and `resolved_text` only a text that starts with `.` and a
  separator. The ids of imports made of a folder, an archive or an ADMX file hash the kept form too
  (`kept_name` of the window). A new file that keeps a path converts the same way.

- W# (`docs/ww.md`): `ww::Window<WinWeaver>` owns `shell: Shell<WinWeaver>` (the tabs, the
  settings, the status, the themes, the session) and `app: WinWeaver` apart, so every method of
  WinWeaver takes `shell: &mut Shell<WinWeaver>` (or `&Shell`) beside `self`; an act of the window
  is `Act::Shell(ShellAct::..)` (open, save, the tabs, the settings, the themes, `Ask` and `Answer`,
  `Message`, `ShowInFolder`, `OpenLink`, `About`), run by the shell, the others by `WinWeaver::apply`. A
  question is `Question::into()` (`Act::Shell(ShellAct::Ask)`), with a second answer by
  `Question::or` (`ShellAct::AnswerAlt`); closing a tab with changes asks Save, Don't save or Cancel
  (`Shell::close_question`, `ShellAct::SaveAndClose`), forgetting the recent files asks through
  `shell.forget_recent_act`. About is a window over the tabs (`App::about` gives its lines). The
  settings tab and the editor of a theme are `Doc::Shell(ShellDoc)`, drawn by the shell; a program
  section of the settings is `SettingsSection` with its `index()` for `ShellAct::OpenSection`. The
  tests build `Window::new(WinWeaver::new(catalog, texts), settings)` and read `w.shell.docs`,
  `w.app.marks`; a method of the app is called as `w.app.m(&mut w.shell, ..)`. The texts of the
  shell (`crates/ww/src`) are in every `ui.json` of WinWeaver (the test reads both crates) and in
  `package-wedit/lang/uk/ui.json`. wedit is built by the release build of the workspace and packaged
  by the job Package as `wedit-<version>.zip` (the exe, its README, `lang/`).

- The settings file: `Settings::load(program)` reads the
  first of `<exe stem>.settings.json`, `<program>.settings.json` and `settings.json` that exists and
  writes back to it (`Shell::settings_path`), so two copies of an executable with different names keep
  their own settings and a plain `settings.json` goes on. `Settings::wrap_lines` folds the lines of
  a text or a page to the width of the view (the setting Wrap lines of General); off, the view scrolls
  sideways (a `layouter` of infinite width in wedit, an infinite width for the page). A page always
  scrolls sideways for a table or a block of code wider than the view, and its code takes the colour
  `code_bg` of the theme. A program adds colours to the themes with `style::install_colors`
  (`ExtraColor`: name, label, light, dark); a theme file overrides them by name, the theme editor lists
  them after the palette, and `style::extra(ui, name)` gives the one of the theme shown. The fields of
  the form Languages (display language, system and user locale) are typed freely with the list of the
  languages of Windows beside them (`forms::locale_field`).

- Markdown pages: WW draws them itself (`ww::markdown`; pulldown-cmark parses): every paragraph, heading and cell is one `LayoutJob`, so text, code (`code_bg`, valign BOTTOM) and
  links share a baseline and wrap at the width of the view (`Settings::wrap_lines`; off, an infinite width
  and the view scrolls sideways); a link is found under the pointer by `Galley::cursor_from_pos`, a link
  `#anchor` scrolls the page to its heading, any other goes to `open_url`, and the shell takes it
  (`ShellAct::OpenLink`): a `.md` or `.txt` file or a file without an extension opens in the program,
  relative to the page (`Shell::linked_file`; WinWeaver shows a text file as it is, `MdDoc::plain`), a
  web link or a file of another kind is asked first and goes to the browser. A page scrolls with the
  arrows, the page keys, Home and End (`mdview::key_offset`), keeps its place a tab (its own scroll id)
  and its selected text when another tab comes in front (`LabelSelectionState` saved and given back). Lists put their markers in a column of their own on the baseline of the first line; blocks of
  code have a copy button; the whole page (its text, its blocks of code, its tables) is as wide as
  `Settings::page_width` (`PageWidth`: the view, or at most so many pixels or characters, never wider
  than the view; Width of pages in General); a table is bands of cells of that width, its columns shared by the
  length of their text and never narrower than their longest word (`markdown::column_widths`), every row
  as high as its highest cell. The scroll bars are solid (`ScrollStyle::solid`): visible whenever the
  content is larger than the view. The screenshots of CI draw the sample page `ww::sample::PAGE` and
  AGENTS.md at several widths and settings in both programs (`13-` to `20-`, `w1-` to `w11-`): look at them
  after every change of the drawing. `Settings::window` (`WindowPlace`) is
  where the window was at the close; `run` opens it there, and the first frame fits it to the monitor
  it opened on (`window::fit_window`: moved inside, shrunk to the monitor, never below
  `WINDOW_MIN_SIZE`); a maximized window keeps the place it had before.

- Icons: a theme names the font of the icons (`icon_font`, `style::ICON_FAMILIES`: Segoe UI Emoji by
  default, or Segoe UI Symbol), loaded after the font of the texts and before Segoe UI Symbol. egui draws a
  glyph in one colour, so with Segoe UI Emoji the icons of the command bar and the tabs are pictures drawn by
  DirectWrite through Direct2D into WIC with the colours of the font (`ww::coloricons`, once a size and a
  colour, as textures; `style::paint_icon`); without them, or when DirectWrite fails, an icon is a glyph.

- The panel of the work in the background (`App::jobs`, `ww::Job`, `shell/jobs.rs`): WinWeaver
  lists the read of this PC, an import, a change of this PC and the search of the registry (with
  its Stop, which stops the search of every registry tab), each with the time since it was first
  seen (`WinWeaver::track_jobs`, `Full::jobs_since`); the panel folds (`ShellAct::FoldJobs`).

- Menus (`ww::shell::menu_item`, `menu_entry`, `sub_menu`): every item keeps the place of an icon
  at its left (`icon_place`, an empty image of the size), painted by `style::paint_icon`, so the
  texts line up with or without one; the menus of WW itself (a tab, the breadcrumb, the recent
  searches, the copy menus of texts) have icons too. The tab row (`shell/tabs.rs`) is a scroll area
  without a bar when the tabs are wider than the window: the wheel scrolls it sideways, a new active
  tab or count scrolls the active one into view, and a button after the new tab lists every tab.
