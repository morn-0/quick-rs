use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const LIB_NAME: &str = "quickjs";

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    let embed = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let quickjs = embed.join("quickjs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let binding = bindgen::builder()
        .header("wrapper.h")
        .allowlist_function("(__)?(JS|js)_.*")
        .allowlist_var("JS_.*")
        .allowlist_type("JS.*")
        .opaque_type("FILE")
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

    fs::copy("static-functions.c", code_path.join("static-functions.c")).unwrap();

    let patch = embed.join("patch");
    for patch in fs::read_dir(patch).unwrap() {
        let patch = patch.unwrap().path();

        #[rustfmt::skip]
        let if_msvc_patch = patch.ends_with("basic_msvc_compat.patch") || patch.ends_with("msvc_alloca_compat.patch");
        let if_msvc = target_os == "windows" && target_env == "msvc";

        if !if_msvc && if_msvc_patch {
            continue;
        }

        #[cfg(feature = "check-overflow")]
        if patch.ends_with("not-check-overflow.patch") {
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

    let quickjs_version = fs::read_to_string(code_path.join("VERSION")).unwrap();
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
        .define(
            "CONFIG_VERSION",
            format!("\"{}\"", quickjs_version.trim()).as_str(),
        )
        // .define("DUMP_LEAKS", None)
        // .define("DUMP_FREE", None)
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
        .opt_level(3)
        .compile(LIB_NAME);
}

fn exists(path: impl AsRef<Path>) -> bool {
    PathBuf::from(path.as_ref()).exists()
}
