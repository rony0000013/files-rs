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

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_app_icon();
    }
}

fn embed_windows_app_icon() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ico_path = manifest_dir.join("assets").join("app_icon.ico");
    println!("cargo:rerun-if-changed={}", ico_path.display());
    if !ico_path.exists() {
        println!("cargo:warning=assets/app_icon.ico missing: skipping executable icon embed");
        return;
    }

    let Some(rc_exe) = find_windows_rc_exe() else {
        println!("cargo:warning=Windows SDK rc.exe not found: skipping executable icon embed");
        return;
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rc_file = out_dir.join("app_icon.rc");
    let res_file = out_dir.join("app_icon.res");

    let ico_escaped = ico_path.to_string_lossy().replace('\\', "/");
    let rc_contents = format!("1 ICON \"{ico_escaped}\"\n");
    if let Err(e) = std::fs::write(&rc_file, rc_contents) {
        println!("cargo:warning=failed to write app_icon.rc: {e}");
        return;
    }

    let status = std::process::Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_file)
        .arg(&rc_file)
        .status();

    match status {
        Ok(st) if st.success() && res_file.exists() => {
            println!("cargo:rustc-link-arg-bin=files-gpui={}", res_file.display());
        }
        Ok(st) => {
            println!("cargo:warning=rc.exe exited with status {st}");
        }
        Err(e) => {
            println!("cargo:warning=failed to run rc.exe ({}): {e}", rc_exe.display());
        }
    }
}

fn find_windows_rc_exe() -> Option<PathBuf> {
    if let Ok(rc) = env::var("RC") {
        let p = PathBuf::from(rc);
        if p.exists() {
            return Some(p);
        }
    }
    if std::process::Command::new("rc.exe")
        .arg("/?")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some(PathBuf::from("rc.exe"));
    }

    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86") => "x86",
        Ok("aarch64") => "arm64",
        _ => "x64",
    };

    let mut roots = Vec::new();
    if let Ok(pf86) = env::var("ProgramFiles(x86)") {
        roots.push(PathBuf::from(pf86).join("Windows Kits").join("10").join("bin"));
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin"));

    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        versions.sort();
        versions.reverse();
        for ver in versions {
            let candidate = ver.join(arch).join("rc.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}
