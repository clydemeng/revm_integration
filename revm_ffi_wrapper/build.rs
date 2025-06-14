fn main() {
    // Allow unresolved symbols coming from Go (provided at runtime).
    println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
} 