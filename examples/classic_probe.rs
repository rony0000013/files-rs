//! Headless probe: enumerate the classic shell menu for a real file.
//! Usage: cargo run --release --example classic_probe
//! Prints item count + labels (+ whether icons were extracted).

use std::path::PathBuf;

fn main() {
    let dir = std::env::temp_dir();
    let file = dir.join("files_rs_classic_probe.txt");
    std::fs::write(&file, "probe").unwrap();
    let paths = vec![file.clone()];
    let entries =
        files_rs::platform::windows::classic_menu::enumerate_classic_menu(&paths, &dir);
    println!("CLASSIC_COUNT={}", entries.len());
    let with_icons = entries.iter().filter(|e| e.icon.is_some()).count();
    println!("CLASSIC_ICONS={}", with_icons);
    for e in entries.iter().take(30) {
        if e.separator {
            println!("  ---");
        } else {
            println!(
                "  [{}] off={} {}{}",
                if e.enabled { "x" } else { " " },
                e.offset,
                e.label,
                if e.icon.is_some() { " (icon)" } else { "" }
            );
            for c in e.children.iter().take(8) {
                if c.separator {
                    println!("      ---");
                } else {
                    println!(
                        "      [{}] off={} {}{}",
                        if c.enabled { "x" } else { " " },
                        c.offset,
                        c.label,
                        if c.icon.is_some() { " (icon)" } else { "" }
                    );
                }
            }
        }
    }
    let _ = std::fs::remove_file(&file);
    let _ = PathBuf::from("x");
}
