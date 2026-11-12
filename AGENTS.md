# AGENTS.md: map of the repository of W#

For agents and developers: where things are, the rules, the commands.
Last update: 11.11.2026.

Repository: https://github.com/supakov/ww (branch `main`). Commits are authored by
`digidult <271767060+digidult@users.noreply.github.com>` (a local `git config` of the clone, never
global). A commit made by an agent ends with
`Co-Authored-By: <agent model> <noreply@anthropic.com>`.

## 1. The project

W# (`ww`) is the framework of the window of programs with document tabs and the system of their
plugins: `ww-base`, `ww`, the runtime `wwrt` (a host of plugins, the cache of the blocks, the
native interface), the MCP server `wwmcp` (the tools of a program and of its plugins), the stable
interface `ww.h` with its crate `ww-stable`, and the examples: the
demo program `ww-demo`, the native template `ww-hello`, the plugins of the stable interface
`ww-mandelbrot` (C) and `ww-grey` (Rust).

## 2. What to read first

| Task | Start with |
|---|---|
| Write a program on W#, or change the shell of the window | `docs/ww.md`, then `crates/ww/src/shell/mod.rs` (the trait `App`) and `examples/ww-demo` |
| Change the native interface of the plugins | `docs/plugins.md`, `crates/wwrt/src/plugin.rs`, `inner.rs`, `menus.rs`, `mime.rs`, `blocks.rs`, `trust.rs` |
| Change the parts of a host of plugins | `docs/plugins.md` (section 8), `crates/wwrt/src/host.rs` |
| Change the stable interface | `docs/stable.md`, `stable/ww.h`, `crates/wwrt/src/stable.rs`, `stable/rust` |
| Change the MCP server, the tools or its tab | `docs/mcp.md`, `crates/wwmcp/src` (`server.rs`, `tools.rs`, `config.rs`, `ui.rs`, `window.rs`), `crates/wwrt/src/tools.rs`, `host.rs` (`HostMcp`) |
| Change the embedded files | `docs/embedded.md`, `crates/ww/src/embedded.rs`, `crates/ww/src/shell/page.rs` |
| Write a plugin | `examples/ww-hello` (native), `examples/ww-mandelbrot` (C), `examples/ww-grey` (Rust of any toolchain) |
| Why the plugins are built so | `docs/design.md` |
| Facts a change must respect | `docs/details.md` |

## 3. Rules

1. Everything in the repository is English: code, comments, documents, commit messages; the
   translations are files of their own (`lang/<code>/`).
2. No em dash (U+2014) and no en dash (U+2013) in any text: number ranges use a hyphen.
3. Dependencies: popular, open source Rust crates that need no extra DLL. A program on W# is one
   executable; a program with plugins adds `wwrt.dll`, `std-<hash>.dll` and the folder `plugins`.
4. Text files are UTF-8 without a byte order mark with LF line endings (CRLF for `.cmd`, `.bat`,
   `.ps1`); `rustfmt` and `clippy -D warnings`.
5. Untrusted input (files, plugins, bytes inside a file, the tables of a plugin) is read with
   limits and turned into a message, never into a crash.
6. Every change reaches `main` through a pull request, which the owner merges. The programs on W#
   follow `main`: a change that breaks one of them is fixed there in the same round.
7. Every text of the interface is `tr("English")` or `trf("English {name}", ...)`; the programs
   translate the texts of W#, of wwrt and of wwmcp (their tests read `ww::SOURCES`,
   `wwrt::SOURCES` and `wwmcp::SOURCES`), the demo in Ukrainian and Russian.
8. An agent changes no setting of the Windows it works on, starts no window there and installs no
   toolchain there without the word of the owner of that computer; the tests draw without a
   screen.
9. The documents hold the current state; the history of the changes is in `docs/releases/`.
10. Versions: `version` of the workspace (0.1.51 on 11.11.2026) grows with every pull request
    that reaches `main`. An agent changes the third number alone; the first and the second
    change only on the word of the owner. `wwrt::plugin::ABI` names it. The interface of the
    plugins (`wwrt::plugin::API`, `WW_API_VERSION`) stays 1 until the first release; the crate
    `ww-stable` keeps the version of its interface (1.x for `ww.h` version 1).

## 4. Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# W# for the browser (rustup target add wasm32-unknown-unknown)
cargo clippy -p ww --no-default-features --features glow --target wasm32-unknown-unknown -- -D warnings
# the plugin of stable Rust, a workspace of its own
(cd examples/ww-grey && cargo fmt --check && cargo clippy --all-targets -- -D warnings)
```

On a computer of Windows, `build-all.cmd` of the root makes what CI makes: the release builds of
the workspace and of `examples/ww-grey`, the plugin in C where a compiler of C is found, and
`dist\ww-demo\` with `dist\ww-demo-<version>.zip`; `build-all.cmd check` runs the checks above
first, all of them.
The tools come from `WW_CARGO`, `WW_PYTHON`, `WW_RUSTC` and `WW_CC`, else from the PATH. The file
keeps CRLF (`.gitattributes`): cmd finds its labels by the offsets of the lines, so it is never
edited while it runs.

The workspace links std dynamically (`.cargo/config.toml`), as the family of the plugins does: its
tests find `std-<hash>.dll` through cargo. Before every commit the checks run on the computer of
the developer. CI (`.github/workflows/ci.yml`) runs on demand alone
(`gh workflow run ci.yml --ref <branch> -R supakov/ww`): the checks on Linux for Windows, then on
Windows the tests, the release builds, the plugin in C built by the compiler of Visual Studio, the
demo listing and drawing its plugins without a window, and the archive of the demo. Agents
editing from Git Bash: a heredoc swallows doubled backslashes, so text with backslashes is written
with the file tools or a script file.

## 5. The programs that take it

The programs on W# take the branch `main` as a git dependency. A change of W# that changes an
interface (the trait `App`, the native or the stable interface of the plugins, `ShellDoc`,
`ShellAct`) is checked against them before its pull request: their checks with the dependency on
the branch of the change (a `[patch]` of the git source to the local clone, then `cargo update`),
and their texts translated for the new literals of `ww`, `wwrt` and `wwmcp`.
