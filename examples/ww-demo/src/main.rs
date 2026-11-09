//! ww-demo, the demo of W# (its embedded guide, `docs/guide.md`): the window of W# with every
//! part of it, and a host of the plugins of the folder `plugins` next to the executable.
//!
//! In a release build the executable has no console (`windows_subsystem`): the command line
//! functions attach to the console of the parent when there is one.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bundle;
#[cfg(test)]
mod tests;
mod texts;
mod widgets;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use wwrt::host::{self, attach_console};

const USAGE: &str = "ww-demo, the demo of W#

  ww-demo [FILE...]          open the window, with the files in tabs
  ww-demo --plugins          list the plugins, the formats they open and their files
  ww-demo --probe FILE       open FILE with its plugin and draw its view in three frames,
                             without a window; print the times
  ww-demo --smoke-window     open a small window, draw two frames, print what drew them
  ww-demo --version          print the version
  ww-demo --help             this text

The plugins are the DLLs of the folder plugins next to ww-demo.exe (or of the folder
WW_DEMO_PLUGINS names), and two plugins the program makes itself.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(String::as_str);
    if matches!(first, Some("--help" | "-h")) {
        attach_console();
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if first == Some("--version") {
        attach_console();
        println!("ww-demo {}", app::VERSION);
        return ExitCode::SUCCESS;
    }
    let dir = host::plugins_dir("WW_DEMO_PLUGINS");
    // The plugins it trusts and the limits of the cache are in the settings, read before any
    // plugin loads.
    let (settings, _) = wwrt::ww::Settings::<app::Extra>::load(<app::Demo as wwrt::ww::App>::NAME);
    let own = settings.map(|s| s.extra.host).unwrap_or_default();
    let started = Instant::now();
    let plugins = app::plugins_of(&dir, &own);
    let took = started.elapsed();
    match first {
        Some("--plugins") => {
            attach_console();
            println!("{}", host::list(&plugins, &dir, took));
            ExitCode::SUCCESS
        }
        Some("--probe") => {
            attach_console();
            let Some(file) = args.get(1) else {
                eprintln!("--probe needs a file\n\n{USAGE}");
                return ExitCode::from(2);
            };
            match host::probe(&plugins, Path::new(file)) {
                Ok(text) => {
                    println!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--smoke-window") => {
            attach_console();
            match wwrt::ww::smoke_window::<app::Demo>(None) {
                Ok(text) => {
                    println!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(a) if a.starts_with("--") => {
            attach_console();
            eprintln!("unknown option {a}\n\n{USAGE}");
            ExitCode::from(2)
        }
        _ => {
            let plugins = Arc::new(plugins);
            let files: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();
            let make = move |_: &_| app::Demo::new(plugins.clone(), files.clone());
            match wwrt::ww::run::<app::Demo>(None, make) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    attach_console();
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
