#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
// bindings.rs 为 bindgen 自动生成，不对其施加 clippy / rustc 风格 lint。
#![allow(clippy::all)]
#![allow(unknown_lints, unnecessary_transmutes)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
