// Link our precompiled artwork resources; normal builds need no resource
// compiler or Cargo dependency. Regenerate them with tools/build_icon_resources.py
// after changing resources/chezcat.rc or assets/chezcat.ico.
use std::{env, path::PathBuf};

#[allow(dead_code)]
#[path = "src/crypto_hash.rs"]
mod crypto_hash;
#[allow(dead_code)]
#[path = "src/license_acceptance.rs"]
mod license_acceptance;

fn main() {
    println!("cargo:rerun-if-changed=LICENSE");
    println!("cargo:rerun-if-changed=src/license_acceptance.rs");
    println!("cargo:rerun-if-env-changed=CHEZCAT_LICENSE_RECEIPT_DIR");
    println!("cargo:rerun-if-env-changed=LOCALAPPDATA");
    let receipt = license_acceptance::directory(None, None).and_then(|path| {
        // Receipt changes must recheck direct Cargo builds even when source is unchanged.
        println!("cargo:rerun-if-changed={}", path.join("receipt.json").display());
        license_acceptance::require(&path)
    });
    if let Err(problem) = receipt { panic!("{problem} Use python tools/accept_license.py after reviewing LICENSE; this build never accepts silently."); }

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
