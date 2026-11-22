fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(rs_alloc_tcmalloc)");
    #[cfg(any(feature = "tcmalloc", feature = "auto"))]
    build_tcmalloc();
}

#[cfg(any(feature = "tcmalloc", feature = "auto"))]
fn build_tcmalloc() {
    use std::{env, path::Path, process::Command};

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let gnu_linux = os == "linux" && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    let automatic = cfg!(feature = "auto")
        && !cfg!(feature = "linux-mimalloc")
        && gnu_linux
        && env::var("CARGO_CFG_TARGET_POINTER_WIDTH").as_deref() == Ok("64");
    if !matches!(arch.as_str(), "x86_64" | "aarch64")
        || !(automatic || cfg!(feature = "tcmalloc") && (gnu_linux || os == "macos"))
    {
        return;
    }

    const SOURCE: &str = "vendor/gperftools-2.19.3.tar.xz";
    println!("cargo:rerun-if-changed={SOURCE}");
    let out = env::var_os("OUT_DIR").unwrap();
    let status = Command::new("tar")
        .args(["-xf", SOURCE, "-C"])
        .arg(&out)
        .status()
        .expect("extracting gperftools requires tar with xz support");
    assert!(status.success(), "unable to extract {SOURCE}");

    let mut config = autotools::Config::new(Path::new(&out).join("gperftools-2.19.3"));
    if env::var("HOST") != env::var("TARGET") {
        config.config_option("host", Some(&env::var("TARGET").unwrap()));
    }
    let dst = config
        .disable_shared()
        .enable_static()
        .with("pic", None)
        .disable("libunwind", None)
        .make_target("libtcmalloc.la")
        .build();
    let lib = dst.join("build/.libs");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=tcmalloc");
    println!(
        "cargo:rustc-link-lib={}",
        if os == "macos" { "c++" } else { "stdc++" }
    );
    println!("cargo:rustc-cfg=rs_alloc_tcmalloc");
}
