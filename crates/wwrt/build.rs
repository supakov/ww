//! The build of the runtime (`plugin::ABI`): Rust has no stable ABI, so a plugin works with the
//! runtime it was built against alone. The compiler, the target and the profile name that build
//! in words. Another version of a crate the runtime holds needs no words: rustc names every symbol
//! by a hash of its crate, so a plugin made against other versions misses the imports of
//! wwrt.dll and does not load (`plugin::load` says so).

use std::process::Command;

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let compiler = Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let target = std::env::var("TARGET").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    println!(
        "cargo:rustc-env=WWRT_BUILD={}; {target}; {profile}",
        compiler.trim()
    );
}
