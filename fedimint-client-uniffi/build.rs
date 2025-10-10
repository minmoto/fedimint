fn main() {
    // This crate uses UniFFI proc macros only (no .udl file)
    // The #[uniffi::export] attributes in src/lib.rs handle code generation
    // This build.rs is kept for potential future build steps
    
    // Trigger rebuild if main source file changes
    println!("cargo:rerun-if-changed=src/lib.rs");
}

