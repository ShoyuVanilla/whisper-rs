#![allow(clippy::uninlined_format_args)]

extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap();
    // Link C++ standard library
    if let Some(cpp_stdlib) = get_cpp_link_stdlib(&target) {
        println!("cargo:rustc-link-lib=dylib={}", cpp_stdlib);
    }
    // Link macOS Accelerate framework for matrix calculations
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=Accelerate");
    }

    println!("cargo:rustc-link-search={}", env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-lib=static=whisper");
    println!("cargo:rerun-if-changed=wrapper.h");

    if env::var("WHISPER_DONT_GENERATE_BINDINGS").is_ok() {
        let _: u64 = std::fs::copy(
            "src/bindings.rs",
            env::var("OUT_DIR").unwrap() + "/bindings.rs",
        )
        .expect("Failed to copy bindings.rs");
    } else {
        let bindings = bindgen::Builder::default()
            .header("wrapper.h")
            .clang_arg("-I./whisper.cpp")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks))
            .generate();

        match bindings {
            Ok(b) => {
                let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
                b.write_to_file(out_path.join("bindings.rs"))
                    .expect("Couldn't write bindings!");
            }
            Err(e) => {
                println!("cargo:warning=Unable to generate bindings: {}", e);
                println!("cargo:warning=Using bundled bindings.rs, which may be out of date");
                // copy src/bindings.rs to OUT_DIR
                std::fs::copy(
                    "src/bindings.rs",
                    env::var("OUT_DIR").unwrap() + "/bindings.rs",
                )
                .expect("Unable to copy bindings.rs");
            }
        }
    };

    // stop if we're on docs.rs
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // build libwhisper.a
        env::set_current_dir("whisper.cpp").expect("Unable to change directory");
        let code = std::process::Command::new("make")
            .arg("libwhisper.a")
            .status()
            .expect("Failed to build libwhisper.a");
        if code.code() != Some(0) {
            panic!("Failed to build libwhisper.a");
        }
        // move libwhisper.a to where Cargo expects it (OUT_DIR)
        std::fs::copy(
            "libwhisper.a",
            format!("{}/libwhisper.a", env::var("OUT_DIR").unwrap()),
        )
        .expect("Failed to copy libwhisper.a");
        // clean the whisper build directory to prevent Cargo from complaining during crate publish
        std::process::Command::new("make")
            .arg("clean")
            .status()
            .expect("Failed to clean whisper build directory");
    }

    #[cfg(target_os = "windows")]
    {
        // generate cmake build system
        env::set_current_dir("whisper.cpp").expect("Unable to change directory");
        let code = std::process::Command::new("cmake")
            .args([
                "-S",
                ".",
                "-B",
                "./build",
                "-DCMAKE_CXX_FLAGS=\"/utf-8\"",
                "-DBUILD_SHARED_LIBS=OFF",
            ])
            .status()
            .expect("Failed to generate cmake build system");
        if code.code() != Some(0) {
            panic!("Failed to generate cmake build system");
        }
        // build whisper.lib
        let args = if cfg!(debug_assertions) {
            [
                "--build",
                "build",
                "--target",
                "whisper",
                "--config",
                "Debug",
                "--",
                "/p:OutDir=output",
            ]
        } else {
            [
                "--build",
                "build",
                "--target",
                "whisper",
                "--config",
                "Release",
                "--",
                "/p:OutDir=output",
            ]
        };
        let code = std::process::Command::new("cmake")
            .args(args)
            .status()
            .expect("Failed to build whisper.lib");
        if code.code() != Some(0) {
            panic!("Failed to build whisper.lib");
        }
        // move whisper.lib to where Cargo expects it (OUT_DIR)
        std::fs::copy(
            "./build/output/whisper.lib",
            format!("{}/whisper.lib", env::var("OUT_DIR").unwrap()),
        )
        .expect("Failed to copy whisper.lib");
        // clean the whisper build directory to prevent Cargo from complaining during crate publish
        #[cfg(windows)]
        std::process::Command::new("cmd")
            .args(["/C", "rd", "/s", "/q", "build"])
            .status()
            .expect("Failed to clean whisper build directory");
        #[cfg(not(windows))]
        std::process::Command::new("sh")
            .args(["-c", "rm", "-rf", "build"])
            .status()
            .expect("Failed to clean whisper build directory");
    }
}

// From https://github.com/alexcrichton/cc-rs/blob/fba7feded71ee4f63cfe885673ead6d7b4f2f454/src/lib.rs#L2462
fn get_cpp_link_stdlib(target: &str) -> Option<&'static str> {
    if target.contains("msvc") {
        None
    } else if target.contains("apple") || target.contains("freebsd") || target.contains("openbsd") {
        Some("c++")
    } else if target.contains("android") {
        Some("c++_shared")
    } else {
        Some("stdc++")
    }
}
