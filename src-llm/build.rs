fn main() {
    // llama.cpp reads the CPU name from the registry on Windows (MinGW does
    // not link advapi32 by default).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // After the static llama.cpp archives that need it.
        println!("cargo:rustc-link-arg=-ladvapi32");
    }
}
