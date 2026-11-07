# AGENTS.md: map of the repository of W#

For agents and developers: where things are, the rules, the commands.
Last update: 07.11.2026.

Repository: https://github.com/supakov/ww (private, branch `main`). Commits are authored by
`digidult <271767060+digidult@users.noreply.github.com>` (a local `git config` of the clone, never
global). A commit made by an agent ends with
`Co-Authored-By: <agent model> <noreply@anthropic.com>`.

## 1. The project

W# (`ww`) is the framework of the window of WinWeaver (digidult/winweaver), wedit and wview
(supakov/wview), and the system of their plugins: `ww-base`, `ww`, the runtime `wwrt`, the stable
interface `wview.h` with its crate `wview-stable`, and the examples of a plugin. It came out of the
repository of WinWeaver on 07.11.2026 (WinWeaver 0.43.0, wview 0.14.0); its history before is
there.

## 2. What to read first

| Task | Start with |
|---|---|
| Write a program on W#, or change the shell of the window | `docs/ww.md`, then `crates/ww/src/shell/mod.rs` (the trait `App`) |
| Change the interface of the plugins | `docs/plugins.md` (6.1, 6.8), `crates/wwrt/src/plugin.rs`, `inner.rs`, `mime.rs`, `trust.rs` |
| Change the stable interface | `docs/plugins.md` (6.9, 6.10), `stable/wview.h`, `stable/README.md`, `crates/wwrt/src/stable.rs`, `stable/rust` |
| Write a plugin | `examples/wv-hello` (native), `examples/wv-mandelbrot` (C), `stable/README.md` (Rust of any toolchain) |

## 3. Rules

1. Everything in the repository is English: code, comments, documents, commit messages.
2. No em dash (U+2014) and no en dash (U+2013) in any text: number ranges use a hyphen.
3. Dependencies: popular, open source Rust crates that need no extra DLL. A program on W# is one
   executable; a program with plugins adds `wwrt.dll`, `std-<hash>.dll` and the folder `plugins`.
4. Text files are UTF-8 without a byte order mark with LF line endings (CRLF for `.cmd`, `.bat`,
   `.ps1`); `rustfmt` and `clippy -D warnings`.
5. Untrusted input (files, plugins, bytes inside a file) is read with limits and turned into a
   message, never into a crash.
6. Every change reaches `main` through a pull request, which the customer merges. The programs
   follow `main`: a change that breaks one of them is fixed there in the same round.
7. Every text of the interface is `tr("English")` or `trf("English {name}", ...)`; the programs
   translate the texts of W# and of wwrt (their tests read `ww::SOURCES` and `wwrt::SOURCES`).
8. The work PC of the customer is only read: no setting of Windows changes there, a program is
   never started there by an agent, and a toolchain is installed there only on the word of its
   owner.
9. The documents hold the current state; the history of the changes is in `docs/releases/`.
10. Versions: `version` of the workspace grows with every pull request that reaches `main`, the
    second number for a feature, the third for fixes alone; `wwrt::plugin::ABI` names it.

## 4. Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace links std dynamically (`.cargo/config.toml`), as the family of the plugins does: its
tests find `std-<hash>.dll` through cargo. Before every commit the three checks run on the computer
of the developer. CI (`.github/workflows/ci.yml`) runs on demand alone, with the word of the user
(`gh workflow run ci.yml --ref <branch> -R supakov/ww`): the checks on Linux for Windows, then on
Windows the tests, the template `wv-hello` and the example in C built by the compiler of Visual
Studio. Agents editing from Git Bash: a heredoc swallows doubled backslashes, so text with
backslashes is written with the file tools or a script file.

## 5. The programs that take it

| Program | Repository | Takes |
|---|---|---|
| WinWeaver, wwlite | digidult/winweaver | `ww-base`, `ww` |
| wview and its plugins | supakov/wview, folder `wview` | `wwrt` (and W# through it), the template and `stable/` for its SDK |
| wedit | supakov/wview, folder `wedit` | `ww` |

Each takes the branch `main` as a git dependency, with the secret `WW_TOKEN` in its CI. A change of
W# is checked against them before its pull request: their checks with the dependency on the branch
of the change (`cargo update` after a `[patch]` of the git source to the local clone).
