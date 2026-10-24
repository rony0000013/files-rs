//! Spike: BetterOffice xlsx/pptx -> PNG pages. Usage:
//!   office_spike <file> <out_dir>
use std::path::Path;

fn system_font(name: &str) -> Option<Vec<u8>> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let candidates = [
        format!("{}\\Fonts\\{}.ttf", windir, name),
        format!("{}\\Fonts\\{}.ttc", windir, name),
    ];
    for c in candidates {
        if let Ok(b) = std::fs::read(&c) {
            return Some(b);
        }
    }
    None
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: office_spike <file> <out_dir>");
    let out = Path::new(args.get(2).map(|s| s.as_str()).unwrap_or("D:/hermes-work/office-spike"));
    std::fs::create_dir_all(out).unwrap();
    let bytes = std::fs::read(path)?;
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    // Register a handful of common families from the OS font directory.
    let mut fonts: Vec<(&str, Vec<u8>)> = Vec::new();
    for fam in ["calibri", "calibrib", "calibrii", "calibriz", "arial", "arialbd", "times", "segoeui"] {
        if let Some(b) = system_font(fam) {
            fonts.push((fam, b));
        }
    }
    println!("fonts loaded: {:?}", fonts.iter().map(|(n, _)| *n).collect::<Vec<_>>());

    match ext.as_str() {
        "xlsx" => {
            let wb = betteroffice_xlsx::Workbook::open(&bytes)?;
            println!("sheets: {:?}", wb.sheet_info().map(|i| i.sheet_names.clone()).unwrap_or_default());
            let vp = betteroffice_xlsx::Viewport { x: 0.0, y: 0.0, width: 1280.0, height: 900.0 };
            let png = wb.render_png(&vp)?;
            println!("xlsx png {} bytes {}x{}", png.bytes.len(), png.width, png.height);
            std::fs::write(out.join("xlsx_sheet.png"), &png.bytes)?;
        }
        "pptx" => {
            let mut pres = betteroffice_pptx::Presentation::open(&bytes)?;
            for (fam, bytes) in &fonts {
                let base = fam.trim_end_matches(['b', 'i']);
                let bold = fam.contains('b');
                let italic = fam.contains('i');
                let _ = pres.register_font(base, bold, italic, bytes);
            }
            println!("slides: {}", pres.slides().len());
            for i in 0..pres.slides().len().min(3) {
                let opts = betteroffice_pptx::RenderOptions::default();
                let slide = pres.render_png(i, &opts)?;
                println!("slide {}: {} bytes", i, slide.bytes.len());
                std::fs::write(out.join(format!("pptx_slide{}.png", i)), &slide.bytes)?;
            }
        }
        _ => {
            println!("unsupported ext for spike: {}", ext);
        }
    }
    println!("done -> {}", out.display());
    Ok(())
}
