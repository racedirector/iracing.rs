use std::{env, path::PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.ends_with("windows-msvc") {
        return;
    }

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set"))
        .join("windows")
        .join("as_invoker.manifest");
    let manifest = manifest
        .to_str()
        .expect("manifest path must be valid UTF-8 for linker args");

    for target_kind in ["bins", "examples"] {
        println!("cargo:rustc-link-arg-{target_kind}=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-{target_kind}=/MANIFESTINPUT:{manifest}");
    }
}
