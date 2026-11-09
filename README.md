# W# (ww)

W# is a framework for Windows programs with document tabs, written in Rust on egui, and a system
of plugins for them. A program on W# is one portable executable with its data folders next to it:
a window with document tabs, a menu, a command bar, a navigation row, a message bar, a status bar,
settings, themes with an editor, translations, drafts of unsaved changes, the tabs of the last
session and pages it carries inside itself. A program with plugins adds the runtime `wwrt.dll` and
a folder `plugins`: native plugins in Rust that draw with the egui of the window, and plugins of a
stable C interface in any language. A program and its plugins serve tools to AI agents over MCP
and call them by their names (`wwmcp`).

The demo, `examples/ww-demo`, shows every part once.

## The parts

| Folder | What |
|---|---|
| `crates/ww-base` | strict JSON files with the trait `Document`, paths inside the program folder, the clock |
| `crates/ww` | the window: the shell and its trait `App`, the style and the themes, the translations, the painted tree and list, the Markdown viewer, a text editor, embedded files, the settings and the session, the browser edition (wasm32) |
| `crates/wwrt` | the runtime of the programs with plugins, a Rust dylib: W# and egui re-exported, the native interface of the plugins, the cache of the blocks of files, the type of some bytes, the trust of a plugin, the stable interface as a plugin, the tools of the plugins, the parts of a host of plugins |
| `crates/wwmcp` | the MCP server: the tools of a program and of its plugins, their switches and options in a `.wwmcp` file, the journal and the log, stdio and loopback HTTP, the command line, the tools of the window, and (feature `ui`) its tab |
| `stable/` | `ww.h`, the stable interface for plugins of any language with a C FFI, and `rust/`, the crate `ww-stable` for such a plugin in Rust of any toolchain |
| `examples/ww-demo` | the demo: a program on W# and a host of plugins, every part once |
| `examples/ww-hello` | the template of a native plugin |
| `examples/ww-mandelbrot` | a plugin of the stable interface in C: the Mandelbrot set |
| `examples/ww-grey` | a plugin of the stable interface in Rust with `ww-stable`: any bytes as a picture |
| `docs/` | `ww.md` (programs), `plugins.md` (native plugins and hosts), `stable.md` (the stable interface and other languages), `mcp.md` (the MCP server and the tools), `embedded.md` (embedded files), `design.md` (why the plugins are built this way), `details.md` (facts for contributors), `releases/` |
| `tools/` | `imports.py`: the DLLs an executable imports, for packing a program with plugins |

## Build and run the demo

Rust from https://rustup.rs (the toolchain msvc needs the Visual Studio Build Tools with
"Desktop development with C++"; the toolchain gnu works too). In the root of the repository:

```bat
build-all.cmd
```

builds the workspace in release, the plugin of stable Rust and, where a compiler of C is found,
the plugin in C, and packs `dist\ww-demo\` and `dist\ww-demo-<version>.zip`: run
`dist\ww-demo\ww-demo.exe`. Without the script:

```bash
cargo build --release --workspace
target/release/ww-demo.exe
```

The demo loads the native template from the folder of the build and opens its guide.

## A program on W#

A program takes the framework as a git dependency of the branch `main`, which it follows while
the framework has no releases of its own:

```toml
[dependencies]
ww = { git = "https://github.com/supakov/ww", branch = "main" }
# a program with plugins takes the runtime, which re-exports W#:
wwrt = { git = "https://github.com/supakov/ww", branch = "main" }
```

`cargo update -p ww -p ww-base` (and `-p wwrt`) takes the last commit of `main`. docs/ww.md tells
how a program is made: its documents, its acts, its settings, `impl ww::App`, `ww::run`. A program
with plugins builds its family with `-C prefer-dynamic` (the `.cargo/config.toml` of this
repository) and ships `std-<hash>.dll` next to `wwrt.dll` (docs/plugins.md, section 9).

## A plugin

- **Native, in Rust** (docs/plugins.md): copy `examples/ww-hello`, write a `Plugin` and a
  `View`, name it with `wwrt::plugin!`, and build it in the tree of the release of its host.
- **Stable, in any language** (docs/stable.md): include `stable/ww.h`, export `ww_plugin_get`,
  build a DLL once; in Rust of any toolchain the crate `stable/rust` (`ww-stable`) writes the
  table.
- **Tools** (docs/mcp.md): either kind may add tools for AI agents and for a call by name
  (`Plugin::tools`, or `ww_tools_get` of `ww.h`), and call the tools of its program by name.

The interface of both kinds is version 1 and stays 1 until the first release of W#: until then
it may change.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`build-all.cmd check` runs them first. The tests draw the windows without a screen
(egui_kittest).

## License

MIT, `LICENSE`.
