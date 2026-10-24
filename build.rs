//! Build script: stage the bundled Pdfium DLL next to the final binary so the
//! PDF preview engine can bind to it at runtime without installers or PATH hacks.
use std::env;
use std::path::PathBuf;

fn main() {
    let dll = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("thirdparty")
        .join("pdfium")
        .join("pdfium.dll");
    if dll.exists() {
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        // OUT_DIR is target/<profile>/build/<pkg>-<hash>/out -> walk up to <profile>.
        if let Some(profile) = out.ancestors().nth(3) {
            let dest = profile.join("pdfium.dll");
            // Copy on every build: cheap (7 MB) and keeps `cargo clean` checkouts working.
            if std::fs::copy(&dll, &dest).is_ok() {
                println!("cargo:warning=staged pdfium.dll -> {}", dest.display());
            }
        }
        // Re-run if the vendored DLL changes.
        println!("cargo:rerun-if-changed=thirdparty/pdfium/pdfium.dll");
    } else {
        println!("cargo:warning=thirdparty/pdfium/pdfium.dll missing: PDF page rendering disabled");
    }
}
