use std::{env, fs, path::PathBuf, process::Command};

const LIB_NAME: &str = "quickjs";

fn main() {
    let embed = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let quickjs = embed.join("quickjs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let target = env::var("TARGET").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    let header = quickjs.join("quickjs.h");
    let mut binding = bindgen::builder()
        .header(header.to_str().unwrap())
        .clang_arg("-std=c11")
        .allowlist_item("(__)?(JS|js)_.*");

    match target_os.as_str() {
        "android" => {
            if let Ok(sysroot) = env::var("CARGO_NDK_SYSROOT_PATH") {
                binding = binding.clang_arg(format!("--sysroot={sysroot}"));
            }
            binding = binding.clang_arg(format!("--target={target}"));
        }
        "ios" => {
            if let Some(sdk) = apple_sdk_path("iphoneos") {
                binding = binding.clang_arg("-isysroot").clang_arg(sdk);
            }
            binding = binding.clang_arg(format!("--target={target}"));
        }
        "windows" if target_env == "msvc" => {
            binding = binding.clang_arg(format!("--target={target}"));
        }
        _ => {}
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
    copy_dir::copy_dir(&quickjs, &code_path).unwrap();
    fs::copy("static-functions.c", code_path.join("static-functions.c")).unwrap();

    let sources = [
        "dtoa.c",
        "libregexp.c",
        "libunicode.c",
        "quickjs.c",
        "static-functions.c",
    ];

    let mut cc = cc::Build::new();
    cc.files(sources.iter().map(|f| code_path.join(f)))
        .define("_GNU_SOURCE", None)
        .std("c11")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-missing-field-initializers")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-variable")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-unused-result")
        .flag_if_supported("-Wno-stringop-truncation")
        .flag_if_supported("-Wno-array-bounds")
        // quickjs.c 的 bigint 代码在 GCC 下有 maybe-uninitialized 误报（上游 issue #453）。
        .flag_if_supported("-Wno-maybe-uninitialized")
        .flag_if_supported("-Wno-format-truncation")
        .flag_if_supported("-Wno-format-zero-length")
        .flag_if_supported("-funsigned-char")
        .opt_level(3);

    if env::var("PROFILE").as_deref() == Ok("release") {
        cc.define("NDEBUG", None);
    }

    match target_os.as_str() {
        "windows" if target_env == "msvc" => {
            cc.define("WIN32_LEAN_AND_MEAN", None);
            cc.define("_WIN32_WINNT", "0x0601");

            cc.flag_if_supported("/experimental:c11atomics");
            if target_arch == "x86" {
                cc.flag_if_supported("/arch:SSE2");
            }
        }
        "android" => {
            println!("cargo:rustc-link-lib=dylib=m");
        }
        _ => {}
    }

    cc.compile(LIB_NAME);
}

fn apple_sdk_path(sdk: &str) -> Option<String> {
    let output = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
