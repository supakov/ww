# W# (ww)

The framework of the window of programs with document tabs, and the system of their plugins. A
program on W# is one executable with its data folders next to it: a window with document tabs, a
menu, a command bar, a status bar, settings, themes, translations, drafts and the tabs of the last
session (`docs/ww.md`). A program with plugins adds `wwrt.dll` and the folder `plugins`
(`docs/plugins.md`).

| Folder | What |
|---|---|
| `crates/ww-base` | strict JSON files with the trait Document, paths inside the program folder, the clock |
| `crates/ww` | the window: the shell and its trait `App`, the style and the themes, the translations, the painted tree and list, the Markdown viewer, the settings and the session, the browser edition (wasm32) |
| `crates/wwrt` | the runtime of the programs with plugins, a Rust dylib: W# and egui re-exported, the native interface of the plugins (version 2: formats with their MIME types, menu items, bytes in memory, views inside a view), the type of some bytes, the trust of a plugin, the stable interface as a plugin |
| `stable/` | `wview.h`, the stable interface (version 1) for plugins of any language with a C FFI, and `rust/`, the crate `wview-stable` for a plugin of it in Rust of any toolchain |
| `examples/wv-hello` | the template of a native plugin: every part a plugin has, once |
| `examples/wv-mandelbrot` | the example of the stable interface in C: the Mandelbrot set |
| `docs/` | `ww.md` (W#), `plugins.md` (the plugins), `releases/` (the history) |

## A program on W#

A program takes the framework as a git dependency of the branch `main`, which it follows while
the framework has no releases of its own:

```toml
[dependencies]
ww = { git = "https://github.com/supakov/ww", branch = "main" }
# a program with plugins takes the runtime, which re-exports W#:
wwrt = { git = "https://github.com/supakov/ww", branch = "main" }
```

`cargo update -p ww` (and `-p ww-base`, `-p wwrt`) takes the last commit of `main`; the CI of a
program does it before every build. The repository is private: cargo fetches it with git
(`net.git-fetch-with-cli = true` in the `.cargo/config.toml` of the program) and the credentials
of git on a computer, with a token in CI (the secret `WW_TOKEN`, a token that reads this
repository; AGENTS.md).

Among the programs on it: wview and wedit (supakov/wview).

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The example in C is built by the compiler of Visual Studio (`stable/README.md`).
