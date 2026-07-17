//! Linker'a `memory.x` bellek haritasini ulastirir.
//!
//! cortex-m-rt'nin `link.x`'i, bizim `memory.x`'imizi `INCLUDE` ile arar ama
//! onu linker'in arama yolunda BULMASI gerekir. Burada dosyayi Cargo'nun
//! `OUT_DIR`'ine kopyalayip o dizini link-search yoluna ekliyoruz. Boylece
//! `.cargo/config.toml`'daki `-Tlink.x` calisirken `memory.x` bulunur.
//!
//! (Bu, tum cortex-m projelerinin kullandigi standart `build.rs` kaliba.)

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());

    // memory.x veya build.rs degisirse yeniden linkle.
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
