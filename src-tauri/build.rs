fn main() {
    #[cfg(windows)]
    steam_dll();
    tauri_build::build()
}

/// The Steamworks client DLL is optional at runtime: delay-load it so the app starts without it, and put
/// the redistributable next to the built exe (dev/test) — the installer ships it via `bundle.resources`.
#[cfg(windows)]
fn steam_dll() {
    use std::{env, fs, path::PathBuf};
    println!("cargo:rerun-if-changed=redist/win64/steam_api64.dll");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:steam_api64.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
    let dll =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("redist/win64/steam_api64.dll");
    // OUT_DIR = <target>/<profile>/build/<crate>-<hash>/out
    if let Some(profile) = PathBuf::from(env::var("OUT_DIR").unwrap())
        .ancestors()
        .nth(3)
    {
        for dir in [profile.to_path_buf(), profile.join("deps")] {
            let _ = fs::copy(&dll, dir.join("steam_api64.dll"));
        }
    }
}
