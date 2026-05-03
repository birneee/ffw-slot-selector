use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../frontend/src");
    println!("cargo:rerun-if-changed=../frontend/static");
    println!("cargo:rerun-if-changed=../openapi.yaml");

    build_frontend();
}

fn build_frontend() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = manifest_dir.join("../frontend/static");

    if out_dir.join("frontend_bg.wasm").exists() {
        return;
    }

    let status = Command::new("cargo")
        .args([
            "build",
            "-p", "frontend",
            "--target", "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .expect("cargo not found");
    assert!(status.success(), "frontend wasm build failed");

    let wasm = manifest_dir.join("../target/wasm32-unknown-unknown/release/frontend.wasm");

    let status = Command::new("wasm-bindgen")
        .args([
            "--target", "web",
            "--out-dir", out_dir.to_str().unwrap(),
            "--out-name", "frontend",
            wasm.to_str().unwrap(),
        ])
        .status()
        .expect("wasm-bindgen not found — run inside nix develop");
    assert!(status.success(), "wasm-bindgen failed");
}
