use std::{env, fs, path::PathBuf};

const LIB_NAME: &str = "quickjs";

fn main() {
    let embed = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let quickjs = embed.join("quickjs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let header = quickjs.join("quickjs.h");
    let header = header.to_str().unwrap();

    let mut binding = bindgen::builder()
        .header(header)
        .clang_arg("-v")
        .clang_arg("-std=c11")
        .allowlist_item("(__)?(JS|js)_.*");

    let is_cargo_ndk = env::var("CARGO_NDK_ANDROID_PLATFORM").is_ok();
    if is_cargo_ndk {
        let sysroot = env::var("CARGO_NDK_SYSROOT_PATH").unwrap();
        let target = env::var("TARGET").unwrap();

        binding = binding
            .clang_arg(format!("--sysroot={sysroot}"))
            .clang_arg(format!("--target={target}"));
    }

    binding
        .generate()
        .unwrap()
        .write_to_file(out_dir.join("bindings.rs"))
        .unwrap();

    let code_path = out_dir.join("quickjs");
    if code_path.exists() {
        fs::remove_dir_all(&code_path).unwrap();
    }
    copy_dir::copy_dir(quickjs, &code_path).unwrap();

    fs::copy("static-functions.c", code_path.join("static-functions.c")).unwrap();
    let sources = [
        "cutils.c",
        "libregexp.c",
        "libunicode.c",
        "quickjs.c",
        "xsum.c",
        "static-functions.c",
    ];

    let mut cc = cc::Build::new();

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    if target_arch == "x86" && target_env == "msvc" {
        env::set_var("MSVC_CFLAGS", "-arch:sse");
        cc.try_flags_from_environment("MSVC_CFLAGS").unwrap();
    }

    if target_env != "msvc" {
        cc.flag_if_supported("-Werror");
    }

    cc.files(sources.iter().map(|f| code_path.join(f)))
        .define("_GNU_SOURCE", None)
        .std("c11")
        .flag_if_supported("-Wextra")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-missing-field-initializers")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-variable")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-array-bounds")
        .flag_if_supported("-Wno-format-truncation")
        .flag_if_supported("-Wno-format-zero-length")
        .flag_if_supported("-funsigned-char")
        .opt_level(3)
        .compile(LIB_NAME);
}
