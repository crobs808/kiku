use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let swift_source = crate_dir
        .join("..")
        .join("..")
        .join("native")
        .join("macos")
        .join("KikuCapturePlugin")
        .join("SystemAudioCaptureHelper.swift");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing out dir"));
    let helper_output = out_dir.join("kiku-system-audio-helper");

    println!("cargo:rerun-if-changed={}", swift_source.display());

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "aarch64".to_owned());
    let swift_target = match target_arch.as_str() {
        "aarch64" => "arm64-apple-macos13.0",
        "x86_64" => "x86_64-apple-macos13.0",
        _ => "arm64-apple-macos13.0",
    };

    let status = Command::new("xcrun")
        .arg("swiftc")
        .arg("-O")
        .arg("-target")
        .arg(swift_target)
        .arg("-framework")
        .arg("Foundation")
        .arg("-framework")
        .arg("ScreenCaptureKit")
        .arg("-framework")
        .arg("CoreMedia")
        .arg("-framework")
        .arg("CoreAudio")
        .arg("-framework")
        .arg("CoreGraphics")
        .arg("-framework")
        .arg("AVFoundation")
        .arg(swift_source.as_os_str())
        .arg("-o")
        .arg(helper_output.as_os_str())
        .status()
        .expect("failed to invoke swiftc for macOS system audio helper");

    if !status.success() {
        panic!("failed to compile macOS system audio helper with swiftc (status: {status})");
    }

    println!(
        "cargo:rustc-env=KIKU_SYSTEM_AUDIO_HELPER_BINARY={}",
        helper_output.display()
    );
}
