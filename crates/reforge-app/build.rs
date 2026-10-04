use std::env;
use std::path::Path;

fn main() {
    // tauri-build embeds its manifest in the app's binary alone. A test binary without the
    // common controls it names does not start on Windows (STATUS_ENTRYPOINT_NOT_FOUND), so the
    // linker embeds the same manifest in every binary of the crate.
    let msvc = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|target| target == "msvc");
    let mut windows = tauri_build::WindowsAttributes::new();
    if msvc {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    let attributes = tauri_build::Attributes::new().windows_attributes(windows);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
}
