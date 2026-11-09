# Embedded files

A program on W# and each of its plugins may carry files inside their binary: the pages of a guide
and their translations, samples, pictures, the translations of their texts. `ww::embedded` finds
them by their path in their set, in the language of the interface, and the window opens them in
tabs; a link of a page leads to another file of the same set.

## 1. A set of files

`ww::embed!` puts files of the crate that names it into the binary at its build. Each is named by
its path relative to the folder of the crate's `Cargo.toml`, which is also its path in the set, or
by a path of its own in the set and the place it is read from:

```rust
use ww::embedded::Embedded;

pub static FILES: Embedded = ww::embed![
    "docs/guide.md",
    "docs/more.md",
    "samples/example.data",
    // a path of its own in the set, read from another place of the crate
    "lang/uk/docs/guide.md" => "translations/uk/guide.md",
    "lang/uk/ui.json" => "translations/uk/ui.json",
]
.with_folder("");
```

A set finds a file in any case, with `\` or `/` between its folders (`get`, `text`), lists the
files of a folder (`under`), and gives a file in a language (`localized`): `lang/<code>/<path>`,
else the file itself, so a page without a translation shows in English. `with_folder` lets a
folder next to the executable stand for the embedded translations: `read(path, code)` takes
`<exe>/<folder>/lang/<code>/<path>` first, so a language added after the build shows its pages
(at most 16 MB a file). A plugin leaves its set without a folder.

`translations()` gives the code and the text of every `lang/<code>/ui.json` of a set, the form
`ww::i18n` takes: a program may name them as its languages, and the host adds the ones of a
plugin to the window when the plugin loads.

## 2. Pages in tabs

`ShellAct::OpenEmbedded(files, "docs/guide.md#a-heading")` opens a file of a set: a Markdown page,
a text or a file without an extension in a tab of the window (`ShellDoc::Page`, drawn by
`ww::mdview` with its search), at the heading after `#`. A tab of the same page comes to the front.
A link of the page leads to the other files of the set relative to the page: `more.md` of
`docs/guide.md` is `docs/more.md`, `../samples/example.data` is `samples/example.data`, and a
link of no file of the set goes on as any link of a page does (a web address after a question).

`App::open_embedded` opens a file of a set; by default a page (`Shell::open_page`) and a message
for any other file. A program with plugins opens the others by the plugin of their type, read
without a copy (`wwrt::host::open_embedded`, `BlockFile::from_static`).

## 3. The files of a plugin

A native plugin gives its set with `Plugin::files`; a view or the plugin opens a page of it with
`Request::OpenEmbedded`:

```rust
static FILES: Embedded = wwrt::ww::embed!["docs/guide.md", "lang/uk/ui.json"];

impl Plugin for MyPlugin {
    fn files(&self) -> Embedded {
        FILES
    }

    fn act(&self, id: &str, host: &mut Host) {
        if id == "guide" {
            host.requests.push(Request::OpenEmbedded(FILES, "docs/guide.md".to_owned()));
        }
    }
}
```

A plugin of the stable interface lists its files in its table (`files`, `file_count`;
docs/stable.md, section 5). The page of the plugins links the pages of every plugin, and
`--plugins` lists their files.

## 4. In the demo

`examples/ww-demo` embeds its guide (four pages, the first in Ukrainian and Russian too), its
samples and the translations of its texts, and names the translations as its languages; the
template of a plugin, `examples/ww-hello`, embeds its guide in three languages, a sample its guide
links and the translations of its texts; the plugin in C embeds a page and a sample.
