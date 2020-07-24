extern crate pb_rs;

use pb_rs::types::FileDescriptor;
use pb_rs::ConfigBuilder;

use std::path::{Path, PathBuf};

fn main() {
    let in_dir = PathBuf::from(::std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("proto");
    println!("cargo:rerun-if-changed={}", in_dir.to_str().unwrap());

    let in_file = in_dir.join("vector_tile.proto");

    println!("cargo:rerun-if-changed={}", in_file.to_str().unwrap());

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir).join("proto");

    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir).unwrap();
    }

    std::fs::DirBuilder::new().create(&out_dir).unwrap();

    let config_builder = ConfigBuilder::new(&[in_file], None, Some(&out_dir), &[in_dir])
        .unwrap()
        .headers(false)
        .single_module(true);

    FileDescriptor::run(&config_builder.build()).unwrap()
}
