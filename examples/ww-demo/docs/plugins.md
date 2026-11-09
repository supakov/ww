# The plugins

The demo is a host of plugins (`wwrt::host`): the DLLs of the folder `plugins` next to the
executable, and two plugins the program makes itself (`src/bundle.rs`, `src/texts.rs`).

| Plugin | Interface | What it shows |
|---|---|---|
| Hello, the template of a plugin | native Rust (`wwrt::plugin`) | the first line of a `.hello` file |
| Mandelbrot set (C) | the stable interface in C (`ww.h`) | the set of a `.mandel` file |
| Bytes as a picture (stable Rust) | the stable interface in Rust of any toolchain (`ww-stable`) | any bytes, a grey pixel a byte |
| Bundles of parts | native, made by the program | the parts of a `.wwbundle` file |
| Texts | native, made by the program | a text of any name |

## A file and its plugin

Open (Ctrl+O) or a drop on the window opens a file by the plugins of its extension, then by the
plugins of the type its first bytes show (`Plugin::sniff`: the template finds a text that starts
with `Hello`, the example in C the lines of a view of the set), then by the plugin of any bytes.
Open the file with in the menu opens it as any format of any plugin.

## Open with and the context menu

A view that offers some bytes has Open with in its bar, in its context menu (the right button)
and in the submenu Context menu of the menu of the window: the plugins of the type of the bytes
first, in the colour of the accent, then every other one to try. A bundle offers the part chosen,
so [the sample bundle](../samples/demo.wwbundle) shows each part inside its own view and opens
any of them in a tab of its own: the hello by the template, the view of the set by the plugin in
C, the bytes by the plugin of stable Rust.

## The cache of the blocks

The bytes of a part travel as a `BlockFile`, a part of the file read in blocks through one cache
of the program (`wwrt::blocks`), never copied: the plugin in C and the one of stable Rust read
them through `ww_source` of `ww.h`. Settings, Plugins sets the size of a block and the budget of
the cache and shows its state; the status bar names the blocks in memory.

## Trust

Settings, Plugins can load only the plugins whose SHA-256 it lists, and those whose signature
Windows trusts; the page of the plugins (in the menu) names where each one comes from, its
formats and its pages.

## The tools and the MCP server

The program and its plugins serve tools to AI agents over MCP and run them by their names
(`wwmcp`, docs/mcp.md of the repository). **MCP server** in the menu of the plugins opens its tab:

- **Server**: the configuration file (`mcp\default.wwmcp` next to the program), Start and Stop of
  the HTTP server of the window on 127.0.0.1 with its token, the mode (read, edit, files), the
  detailed log and **Open the log**, which opens the log of today, a file of JSON Lines.
- **Tools**: every source with its switch, the options of its card and its tools by groups, each
  with its switch. The window gives `list_files`, `open_file`, `close_file` and `save_file`, the
  demo `new_note`, the template `hello_greet`, `hello_count` and `hello_sample` (its options
  Greeting and May open tabs of the window), the plugin in C `mandelbrot_escape`,
  `mandelbrot_ascii` and `mandelbrot_open` (its option limit), the plugin of stable Rust
  `grey_cache`.
- **Call a tool**: any tool by its name with its arguments, as a script or a plugin would; the
  answer comes below and the call into the journal, with its details at the right.

Without the window, `ww-demo --mcp stdio` serves the tools of the plugins to a client that starts
the program itself (`ww-demo --help` names the options). The sources: `app.rs` (`HostMcp`,
`program_tool`), `examples/ww-hello`, `examples/ww-mandelbrot/mandelbrot.c`,
`examples/ww-grey`.

Back to [the guide](guide.md).
