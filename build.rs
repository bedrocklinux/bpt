use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

// When building a binary, Rust sets $TARGET to the architecture.  Use this to store the binary's
// architecture within the binary.  Used to check the host machine native architecture.
fn generate_typed_host_arch() {
    let mut target = std::env::var("TARGET").expect("Expected TARGET to be set");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("Expected OUT_DIR to be set"));

    // Remove libc reference; does not matter for libc-agnostic bpt architecture description.
    for libc in ["gnu", "musl", "uclibc"] {
        if let Some(start) = target.find(libc) {
            target.replace_range(start..start + libc.len(), "*")
        }
    }

    // Extract libc-agnostic bpt name for architecture
    let target = match target.as_str() {
        // Special case targets with multiple ABI variations
        "arm-unknown-linux-*eabi" => "armv7l",
        "arm-unknown-linux-*eabihf" => "armv7hl",
        // Common case
        _ => target
            .split('-')
            .next()
            .expect("Expected TARGET to include `-`"),
    };

    // Save untyped, raw string
    let path: PathBuf = [&out_dir, Path::new("host-arch")].into_iter().collect();
    File::create(path)
        .expect("Expected to be able to open host arch file")
        .write_all(target.as_bytes())
        .expect("Expected to be able to write host arch file");

    // Prefix enum type
    let target = ["crate::metadata::Arch::", target]
        .into_iter()
        .collect::<String>();

    // Save typed string
    let path: PathBuf = [&out_dir, Path::new("host-arch.rs")].into_iter().collect();
    File::create(path)
        .expect("Expected to be able to open host arch file")
        .write_all(target.as_bytes())
        .expect("Expected to be able to write host arch file")
}

fn main() {
    generate_typed_host_arch();
}
