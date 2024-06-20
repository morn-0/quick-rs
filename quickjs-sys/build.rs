use std::{env, fs, path::PathBuf, process::Command};

const LIB_NAME: &str = "quickjs";

fn main() {
    let embed = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let quickjs = embed.join("quickjs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let header = quickjs.join("quickjs.h");
    let header = header.to_str().unwrap();

    let binding = bindgen::builder()
        .header(header)
        .allowlist_function("(__)?(JS|js)_.*")
        .allowlist_var("JS_.*")
        .allowlist_type("JS.*")
        .generate()
        .unwrap();
    binding.write_to_file(out_dir.join("bindings.rs")).unwrap();

    let code_path = out_dir.join("quickjs");
    if code_path.exists() {
        fs::remove_dir_all(&code_path).unwrap();
    }
    copy_dir::copy_dir(quickjs, &code_path).unwrap();

    fs::copy("static-functions.c", code_path.join("static-functions.c")).unwrap();

    let patch = embed.join("patch");
    for patch in fs::read_dir(patch).unwrap() {
        let patch = patch.unwrap().path();

        #[cfg(not(feature = "mimalloc"))]
        if patch.ends_with("support-rust-malloc.patch") {
            continue;
        }

        Command::new("patch")
            .current_dir(&code_path)
            .arg("-i")
            .arg(patch)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    let sources = [
        "cutils.c",
        "libbf.c",
        "libregexp.c",
        "libunicode.c",
        "quickjs.c",
        "static-functions.c",
    ];

    cc::Build::new()
        .files(sources.iter().map(|f| code_path.join(f)))
        .define("_GNU_SOURCE", None)
        .define("CONFIG_BIGNUM", None)
        .define("CONFIG_MODULE_EXPORT", None)
        .std("c11")
        .flag_if_supported("-Werror")
        .flag_if_supported("-Wextra")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-missing-field-initializers")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-variable")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-array-bounds")
        .flag_if_supported("-Wno-format-truncation")
        .flag_if_supported("-funsigned-char")
        .opt_level(2)
        .compile(LIB_NAME);
}
