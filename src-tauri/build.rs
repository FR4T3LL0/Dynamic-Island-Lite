fn main() {
    // Tauri's optional VC-runtime shim explicitly selects a dynamic UCRT.
    // When Rust already links the complete CRT statically, leave those linker
    // libraries to Rust instead of overriding them with the shim.
    let full_static_crt = std::env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|feature| feature == "crt-static");
    let windows = tauri_build::WindowsAttributes::new().static_vc_runtime(!full_static_crt);
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("Tauri-Buildkonfiguration vorbereiten");
}
