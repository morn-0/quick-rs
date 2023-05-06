use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const LIB_NAME: &str = "quickjs";

fn main() {
    let embed = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let quickjs = embed.join("quickjs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let binding = bindgen::builder()
        .header("wrapper.h")
        .allowlist_function("(__)?(JS|js)_.*")
        .allowlist_var("JS_.*")
        .allowlist_type("JS.*")
        .opaque_type("FILE")
        .opaque_type("JSValue")
        .blocklist_type("FILE")
        .blocklist_function("JS_DumpMemoryUsage")
        .generate()
        .unwrap();
    binding.write_to_file(out_dir.join("bindings.rs")).unwrap();

    let code_path = out_dir.join("quickjs");
    if exists(&code_path) {
        fs::remove_dir_all(&code_path).unwrap();
    }
    copy_dir::copy_dir(quickjs, &code_path).unwrap();

    fs::copy("static-function.c", code_path.join("static-function.c")).unwrap();

    let patch = embed.join("patch");
    for patch in fs::read_dir(patch).unwrap() {
        Command::new("patch")
            .current_dir(&code_path)
            .arg("-i")
            .arg(patch.unwrap().path())
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    let quickjs_version = fs::read_to_string(code_path.join("VERSION")).unwrap();
    cc::Build::new()
        .files(
            [
                "cutils.c",
                "libbf.c",
                "libregexp.c",
                "libunicode.c",
                "quickjs-libc.c",
                "quickjs.c",
                "static-function.c",
            ]
            .iter()
            .map(|f| code_path.join(f)),
        )
        .define("_GNU_SOURCE", None)
        .define(
            "CONFIG_VERSION",
            format!("\"{}\"", quickjs_version.trim()).as_str(),
        )
        .define("CONFIG_BIGNUM", None)
        .define("CONFIG_MODULE_EXPORT", None)
        .flag_if_supported("/std:c11")
        .flag_if_supported("-Wchar-subscripts")
        .flag_if_supported("-Wno-array-bounds")
        .flag_if_supported("-Wno-format-truncation")
        .flag_if_supported("-Wno-missing-field-initializers")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wundef")
        .flag_if_supported("-Wuninitialized")
        .flag_if_supported("-Wunused")
        .flag_if_supported("-Wwrite-strings")
        .flag_if_supported("-funsigned-char")
        .flag_if_supported("-Wno-cast-function-type")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-enum-conversion")
        .flag_if_supported("-Wunknown-pragmas")
        .opt_level(2)
        .compile(LIB_NAME);
}

fn exists(path: impl AsRef<Path>) -> bool {
    PathBuf::from(path.as_ref()).exists()
}
