# Why the plugins of W# are built this way

The study that chose the way of the plugins of W#, its measurements and what it found. The
interface itself is in docs/plugins.md (native plugins) and docs/stable.md (the stable interface).

## 1. The question

A program loads the DLLs of a folder, written in Rust as the program is. A plugin should hold its
own code alone: std, egui, eframe and W# belong to DLLs every program of this kind shares, which is
the exception such a program makes to the rule of one executable. The way should stay as close to
ordinary Rust as it can: traits, `egui::Ui`, the types of W#, no C layer in between.

## 2. The ways Rust has

| Way | The plugin draws in the window of the host | Base code in each plugin | ABI | Verdict |
|---|---|---|---|---|
| A runtime as a Rust `dylib`, the plugins `cdylib` on it | yes: one egui, one `Ui`, the traits of W# | none | the compiler and the lock file of the build | the native plugins |
| `cdylib` with a C ABI | no: each plugin has its own egui, its own statics; a `Ui` of the host cannot cross | all of it, about 10 MB a plugin | stable | the stable interface, for plugins that give data |
| `abi_stable`, `stabby` | no: only their stable types cross | all of it | stable, checked at the load | the same |
| WebAssembly (wasmtime, extism) | no: the plugin describes, the host draws | a runtime of 10 MB and more | stable, a sandbox | for plugins nobody vouches for, later |
| A process a plugin, IPC | no | a whole program | stable, isolated | for crashes and rights apart |
| Descriptions as data (JSON) | not code | none | the format of the file | inside a plugin: formats described as data |

Rust has no stable ABI, so the first way needs one build of the host, the runtime and the plugins:
one compiler, one `Cargo.lock`, one profile. For a family of programs and plugins built in one
tree that is no loss, and it is the only way where a plugin is its own code alone and draws with
the egui of the program. Plugins of others take the second way: a stable interface of C
(`stable/ww.h`), whose plugin draws a picture and names its controls.

## 3. Measurements

On 04.11.2026, on a computer of development: Rust 1.99.1, the target `x86_64-pc-windows-gnu`,
half of an i5-1035G1, the release profile of the family (no LTO, one codegen unit, stripped), a
viewer of files with a plugin of texts and one of images.

| File | Size | It needs |
|---|---|---|
| the program | 477 KB | `wwrt.dll`, `std-*.dll` |
| `wwrt.dll` | 20.9 MB | `std-*.dll` |
| `std-f903e3d16df36da5.dll` | 5.3 MB | the toolchain gnu also `libgcc_s_seh-1.dll` (142 KB) and `libwinpthread-1.dll` |
| the plugin of texts | 27 KB | `wwrt.dll`, `std-*.dll` |
| the plugin of images | 52 KB | `wwrt.dll`, `std-*.dll` |

A program of W# linked statically with LTO weighs 12.9 MB; a plugin with its own copy of egui
would weigh about as much (an estimate, not built).

| What | Time |
|---|---|
| Two plugins loaded (the folder read, the DLLs, the checks, the list of the decoders of WIC) | 0.9 ms |
| A Markdown page of 96 KB opened / three frames drawn | 12 ms / 78 ms |
| A PNG of 640 x 480 | 13 ms / 2 ms |
| A PNG of 9000 x 2000, decoded at 8192 x 1820 | 144 ms / 46 ms |
| A whole release build | 9 min 10 s |
| A plugin rebuilt after a change, debug (finding 11) | 50-55 s |

The folder of the program ran with a PATH of `C:\Windows\System32` and `C:\Windows` alone. A DLL
that is no plugin and a plugin whose build differs were refused with their reasons, and Windows
showed no dialog.

## 4. What the study found

1. **One std for the family.** A `dylib` takes std into itself, and an executable that links it
   wants std of its own: rustc stops with `cannot satisfy dependencies so std only shows up once`.
   Every crate of the family is built with `-C prefer-dynamic` (`.cargo/config.toml`), std becomes
   `std-<hash>.dll` of the toolchain, and it ships next to the executable.
2. **The 65535 exports of a DLL.** A `dylib` exports every public symbol of the crates it holds,
   and the export table of a PE file numbers its entries in 16 bits. A debug `wwrt.dll` wanted
   128973 exports. A debug build links when the dependencies and wwrt are optimized
   (`opt-level = 2` for `"*"` and `wwrt` in the profile `dev`): 17600 exports. When the runtime
   grows past the limit, it splits into two DLLs or takes fewer features of `windows`.
3. **No LTO across the runtime.** rustc runs LTO for executables, cdylibs and static libraries,
   never for a `dylib`; with `lto = "thin"` every plugin stops with `cannot prefer dynamic linking
   when performing LTO`. The family has a release profile of its own.
4. **Every shared crate through the runtime.** A plugin with a dependency of its own on a crate
   wwrt holds (serde, egui, windows) would bring a second copy, which rustc refuses; it takes
   `wwrt::serde` and so on. A derive of serde names its crate: `#[serde(crate = "wwrt::serde")]`.
   A crate wwrt does not hold (a parser of a format) is the plugin's own.
5. **One build, checked.** `plugin::ABI` is the build of wwrt: its version, the version of the
   interface, the compiler, the target and the profile (`wwrt/build.rs`). rustc names every
   symbol by a hash of its crate, so other versions of the shared crates fail at the load anyway.
   A plugin built against another `wwrt.dll` misses its imports and fails in `LoadLibrary`; the
   loader sets the error mode of its thread, so Windows shows no dialog and the list of the
   plugins names the reason.
6. **A plugin is never unloaded.** Its code and the tables of its traits live as long as the views
   it made; `Plugins::load_dir` forgets the `Library`.
7. **COM belongs to the window.** winit takes the apartment of the main thread for drag and drop; a
   plugin that asks COM before the window does it in a thread of its own.
8. **The toolchain gnu brings MinGW.** Its `dylib` and `std-*.dll` link the unwinder of MinGW
   dynamically: `libgcc_s_seh-1.dll` and `libwinpthread-1.dll` ship too. The toolchain msvc needs
   none of them, and each module takes the CRT statically (`+crt-static`).
9. **A plugin has the rights of the program.** It is native code in the process; there is no
   sandbox. The settings may admit only the plugins whose SHA-256 or signature they list.
10. **A panic of a plugin unwinds into the host**, since both have one std.
11. **A link costs the import libraries of Windows.** A plugin changed rebuilds in about 50 s in a
    debug build, and nearly none of it is the plugin: rustc writes an import library of each of
    the 68 DLLs of Windows the crates in wwrt call through `raw-dylib`, and the toolchain gnu runs
    `dlltool` and `as` for each one.
12. **A native plugin loads when the host and the plugin are built in one tree.** Cargo hashes a
    path dependency relative to the root of the workspace only when it is inside it, else by its
    absolute path, and rustc names the symbols of `wwrt.dll` by those hashes. A host that takes W#
    as a git dependency publishes the tree of its release, every crate inside its root, and a
    plugin built in a copy of that tree anywhere loads into the release (docs/plugins.md,
    section 9).
