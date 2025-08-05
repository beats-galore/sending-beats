fn main() {
    // Statically link to libmp3lame (Homebrew default path for Apple Silicon)
    println!("cargo:rustc-link-lib=static=mp3lame");
    println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
    tauri_build::build();
}
