// Link our precompiled artwork resources; normal builds need no resource
// compiler or Cargo dependency. Regenerate them with tools/build_icon_resources.py
// after changing resources/chezcat.rc or assets/chezcat.ico.
use std::{env, path::PathBuf};

fn main() {
    for file in [
        "assets/chezcat.ico",
        "resources/chezcat.rc",
        "resources/chezcat.res",
        "resources/chezcat-x64.o",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
        Ok("x86_64"),
        "The desktop build currently supports Windows x64 only."
    );
    let name = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => "chezcat.res",
        Ok("gnu") => "chezcat-x64.o",
        _ => panic!("Windows builds require the MSVC or GNU Rust target."),
    };
    let resource = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("resources")
        .join(name);
    assert!(
        resource.is_file(),
        "Missing resource file: {}",
        resource.display()
    );
    println!("cargo:rustc-link-arg={}", resource.display());
    // Nested EventTarget callbacks add real interpreter/host frames. Measured
    // validation reaches the existing call guard with this virtual reserve;
    // the old 1 MiB prototype overflowed. Work/time guards stay independent.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/STACK:8388608");
    } else {
        println!("cargo:rustc-link-arg=-Wl,--stack,8388608");
    }
}
