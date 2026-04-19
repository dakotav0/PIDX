fn main() {
    // Required by Tauri's `generate_context!()` macro.
    // Registers Tauri's build-time code generation and sets OUT_DIR.
    tauri_build::build()
}
