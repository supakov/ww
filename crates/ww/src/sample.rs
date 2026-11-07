//! A Markdown page that shows every part the page view draws: paragraphs before and after a
//! table, a table with long cells, inline code, a block of code wider than any window, a list.
//! The screenshot tests of the programs draw it at several widths and settings.

/// The text of the page.
pub const PAGE: &str = "# A sample page

The first paragraph is long enough to wrap on a window of any width: it holds `inline code`, \
**strong words** and a [link](https://example.com), and it goes on to show that every text of \
the page has the width of the view, before a table and after it, at the same right edge, when \
the lines wrap.

| Area | State | Date |
|---|---|---|
| Specification | The scope and the interface recorded in `docs/spec/01-ui.md` and the files below, a long cell that has to wrap over several lines in its column | 14.10.2026 |
| Window | Tabs, menu, the command bar of each tab with icons above labels, navigation, search per tab, the painted tree and lists with the keyboard, the detail panel with copy and selectable texts, forms, check, build, the diff tab with comments, bookmarks of options, Markdown tabs with anchors, This PC, the MCP tab with its journal, settings, translations, drafts, restored tabs, the log, themes with fonts and their editor | 15.10.2026 |
| Short | one word | 16.10.2026 |

The paragraph after the table is as long as the first one and must wrap at the same width: \
a table wider than the text must not move the right edge of the text that follows it, and a \
row of the table is as high as its highest cell.

```
cargo build --release -p winweaver --no-default-features --features wgpu && copy target\\release\\winweaver.exe C:\\WinWeaver\\WinWeaver\\winweaver.exe && C:\\WinWeaver\\WinWeaver\\winweaver.exe --smoke-window
```

- A list item with `code` in it.
- A second item, long enough to wrap on a narrow window so that the indent of its second line shows.
  - A nested item under the second one.

1. A numbered step whose number stands on the baseline of its first line.
2. The second step, with `inline code`, a [link to the heading below](#the-last-heading) and *emphasis*.

> A quote: its bar on the left, its text wrapped like a paragraph.

---

## The last heading

The end of the sample page.
";
