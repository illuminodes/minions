use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let service_worker_path = Path::new(&manifest_dir).join("src").join("service_worker");

    // Build the service worker
    Command::new("wasm-pack")
        .current_dir(&service_worker_path)
        .args(&["build", "--target", "web"])
        .status()
        .expect("Failed to build service worker");

    // Copy the WASM file to the dist directory
    std::fs::copy(
        service_worker_path.join("pkg").join("minions_service_worker_bg.wasm"),
        Path::new(&manifest_dir).join("dist").join("serviceWorker_bg.wasm"),
    ).expect("Failed to copy service worker WASM");

    // Create the new serviceWorker.js that loads the WASM module
    let service_worker_content = r#"
    importScripts('./serviceWorker_bg.js');
    wasm_bindgen('./serviceWorker_bg.wasm').then(() => {
        wasm_bindgen.init_service_worker();
    });
    "#;

    std::fs::write(
        Path::new(&manifest_dir).join("dist").join("serviceWorker.js"),
        service_worker_content
    ).expect("Failed to create new serviceWorker.js");

    // Copy the generated JS file
    std::fs::copy(
        service_worker_path.join("pkg").join("minions_service_worker.js"),
        Path::new(&manifest_dir).join("dist").join("serviceWorker_bg.js"),
    ).expect("Failed to copy service worker JS");

    println!("cargo:rerun-if-changed=src/service_worker");
}