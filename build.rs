//! On macOS: the Swift runtime rpath that `screencapturekit`'s own build script cannot
//! give a dependent package, and, with only the command line tools installed, the
//! Swift library directory it looks for in Xcode's layout (tasks/4-macos-capture,
//! decision 15).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
        if let Ok(out) = std::process::Command::new("xcode-select")
            .arg("-p")
            .output()
        {
            let dir = format!(
                "{}/usr/lib/swift/macosx",
                String::from_utf8_lossy(&out.stdout).trim()
            );
            if std::path::Path::new(&dir).is_dir() {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }
}
