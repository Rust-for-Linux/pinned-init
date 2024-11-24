fn main() {
    // Add a custom cfg() to avoid repeated occurrences of
    // any(feature = "std", feature = "alloc)
    if cfg!(any(feature = "std", feature = "alloc")) {
        println!("cargo::rustc-cfg=HAVE_ALLOCATION");
    }
    println!("cargo::rustc-check-cfg=cfg(HAVE_ALLOCATION)");
    println!("cargo::rerun-if-changed=build.rs");
}
