use rustc_version::{version, Version};

fn main() {
    println!("cargo::rustc-check-cfg=cfg(RUSTC_LINT_REASONS_IS_STABLE)");
    if version().unwrap() >= Version::parse("1.81.0").unwrap()
        || version().unwrap() >= Version::parse("1.81.0-nightly").unwrap()
    {
        println!("cargo:rustc-cfg=RUSTC_LINT_REASONS_IS_STABLE");
    }
}
