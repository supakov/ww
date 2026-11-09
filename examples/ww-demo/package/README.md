# The demo of W#

`ww-demo.exe` shows every part of W#, the framework of the window, and of the system of its
plugins: tabs, menus, the command bar, the navigation row, the message and status bars, the work
in the background, questions, settings, themes, translations, drafts, embedded pages, and a host
of plugins with Open with, context menus, views inside views and the cache of the blocks. Its
guide opens at the first start and with F1.

| File | What |
|---|---|
| `ww-demo.exe` | the program |
| `wwrt.dll`, `std-*.dll` | the runtime of W# and of its plugins, and the standard library of Rust they share (the toolchain gnu adds `libgcc_s_seh-1.dll` and `libwinpthread-1.dll`) |
| `plugins\ww_hello.dll` | the template of a native plugin in Rust |
| `plugins\ww_mandelbrot.dll` | a plugin of the stable interface in C (when a compiler of C built it) |
| `plugins\ww_grey.dll` | a plugin of the stable interface in Rust of any toolchain |
| `samples\` | a view of the Mandelbrot set and a bundle of parts to open |

The program writes its settings (`ww-demo.settings.json`), its drafts and its logs next to the
executable, nothing elsewhere.

```
ww-demo.exe [FILE...]        the window, with the files in tabs
ww-demo.exe --plugins        the plugins, their formats and their files
ww-demo.exe --probe FILE     a file drawn by its plugin in three frames without a window
ww-demo.exe --smoke-window   a small window, two frames
```
