# The window

A program on W# implements the trait `ww::App` (`src/app.rs`): its documents, its acts, its
settings and what it draws inside a tab; the window does the rest.

## The tabs

- **A note** (New note, Ctrl+N, or a `.txt` file) is a text of the program itself in the editor
  of W# (`ww::textdoc::TextDoc`): Save and Save as write it, a change makes a draft two seconds
  later, and the next start brings the draft back when the window closed before a save. Closing a
  note with changes asks Save, Don't save or Cancel.
- **Widgets** (`src/widgets.rs`) holds the painted tree with a context menu on its rows
  (`ww::tree::show_with_menu`) and the painted list (`ww::list::show`) of the folder chosen; its
  navigation row has Back and Forward, the breadcrumb and the star; Check fills the message bar.
- **A page** of this guide is a document of the window (`ShellDoc::Page`); a `.md` file opens
  as a page too.
- **Settings** (Ctrl+comma) has General, the section Demo of the program and the section Plugins.

## The bars

- The command bar holds the commands of the tab in front: commands, switches drawn pressed while
  they are on, and menu buttons (`ww::commands`). Below 1100 pixels it moves into the navigation
  row as icons.
- The status bar shows the texts of the program at its left (the plugin of the tab, the line of
  its view), what the program draws at its right (the blocks in the cache), the message of the
  last act and a spinner while work runs in the background.

## The rest

- **Start a job** runs a count in a thread; the panel of the work over the lower right corner
  shows it with its Stop, and the spinner turns until it ends.
- **Ask a question** asks with two answers beside Cancel (`ww::Question::or`).
- **About** lists the version, the settings file, what draws the window, the runtime of the
  plugins and their count.
- The theme has a colour of the program, Star of the demo (`ww::style::install_colors`), which
  the editor of a theme lists after the palette; Widgets paints the star with it.
- Every text of the window is English with a translation in Ukrainian and Russian embedded
  (Settings, General, Language); a file `lang/<code>/ui.json` next to the executable adds a
  language.

Back to [the guide](guide.md).
