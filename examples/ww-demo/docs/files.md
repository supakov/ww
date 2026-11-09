# Embedded files

The demo carries its guide, its translations and its samples inside its executable
(`ww::embed!` in `src/app.rs`), and every plugin may carry its own (`Plugin::files`):

```rust
static FILES: Embedded = ww::embed![
    "docs/guide.md",
    "lang/uk/docs/guide.md",
    "samples/demo.wwbundle",
    "lang/uk/ui.json",
];
```

- A page of a set opens in a tab of the window (`ShellAct::OpenEmbedded`); a link of the page
  leads to another file of the set, relative to the page: [the window](window.md) is
  `docs/window.md`, [the sample notes](../samples/notes.txt) is `samples/notes.txt`.
- The page shows in the language of the interface: `lang/uk/docs/guide.md` stands for
  `docs/guide.md` in Ukrainian, and a page without a translation shows in English (this one).
  A file `lang/<code>/docs/guide.md` next to the executable stands for both
  (`Embedded::with_folder`).
- Any other file of a set opens by the plugin of its type, read without a copy
  (`BlockFile::from_static`): [the sample bundle](../samples/demo.wwbundle).
- The files `lang/<code>/ui.json` of a set are the translations of its texts: the demo gives them
  to the window as its languages, and a plugin's are added to the window when it loads.

The page of the plugins links the pages of each plugin; the menu of the template has Guide of
the plugin, a page of its own set in three languages.

Back to [the guide](guide.md).
