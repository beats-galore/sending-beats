fn main() {
    // Statically link to libmp3lame (Homebrew default path for Apple Silicon)
    println!("cargo:rustc-link-lib=static=mp3lame");
    println!("cargo:rustc-link-search=native=/opt/homebrew/lib");

    // Link ScreenCaptureKit Swift library (macOS only)
    #[cfg(target_os = "macos")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let swift_lib_path = format!("{}/../src-swift/build", manifest_dir);
        println!("cargo:rustc-link-search=native={}", swift_lib_path);
        println!("cargo:rustc-link-lib=dylib=screencapture_audio");

        // Add rpath so the dylib can be found at runtime
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Resources");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", swift_lib_path);

        // Rerun if Swift library changes
        println!("cargo:rerun-if-changed=../src-swift/ScreenCaptureAudio.swift");
        println!("cargo:rerun-if-changed=../src-swift/build/libscreencapture_audio.dylib");
    }

    tauri_build::build();
}
