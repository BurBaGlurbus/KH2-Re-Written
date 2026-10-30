//! Embeds the private sheet of KH3's executable constants when this machine has one (`KH3_CONSTANTS`, or
//! `kh2-kh3-ref/kh3-constants.tsv` in the home folder). It is never in Git: without it the build is the same but the
//! KH3 ruleset reports that it is unavailable.
use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KH3_CONSTANTS");
    let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"));
    let path = env::var_os("KH3_CONSTANTS").map(PathBuf::from).or_else(|| {
        home.map(|h| {
            PathBuf::from(h)
                .join("kh2-kh3-ref")
                .join("kh3-constants.tsv")
        })
    });
    let text = path
        .inspect(|p| println!("cargo:rerun-if-changed={}", p.display()))
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_default();
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(out.join("kh3_constants.tsv"), text).expect("OUT_DIR is writable");
}
