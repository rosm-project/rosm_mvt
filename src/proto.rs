#![allow(non_snake_case)]
#![allow(unused_imports)]

pub mod vector_tile {
    include!(concat!(env!("OUT_DIR"), "/proto/vector_tile.rs"));
}
