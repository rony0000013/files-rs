//! Headless probe: Office IPreviewHandler COM chain without the app.
use windows::core::{w, HSTRING, Interface};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::IPreviewHandler;
use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithFile;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_OVERLAPPEDWINDOW,
};

fn clsid_for(ext: &str) -> Option<windows::core::GUID> {
    let hkcr = winreg::RegKey::predef(winreg::enums::HKEY_CLASSES_ROOT);
    for key in [
        "8895b1c6-b41f-4c1c-a562-0d564250836f",
        "8895b1c6-b41f-4c1c-a562-0d564af7f68d",
    ] {
        let path = format!(".{}\\shellex\\{{{}}}", ext, key);
        if let Ok(k) = hkcr.open_subkey(&path) {
            if let Ok(clsid) = k.get_value::<String, _>("") {
                let hex: String = clsid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                if hex.len() == 32 {
                    if let Ok(v) = u128::from_str_radix(&hex, 16) {
                        return Some(windows::core::GUID::from_u128(v));
                    }
                }
            }
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).cloned().unwrap_or_else(|| r"D:\files-rs\README.md".to_string());
    let ext = std::path::Path::new(&path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    println!("step0: file={} ext={}", path, ext);

    let sta = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    println!("step1: CoInitializeEx(STA) = {:?}", sta);

    let clsid = match clsid_for(&ext) {
        Some(g) => {
            println!("step2: CLSID = {:?}", g);
            g
        }
        None => {
            println!("step2: FAIL no CLSID");
            return;
        }
    };

    let handler: IPreviewHandler =
        match unsafe { CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER) } {
            Ok(h) => {
                println!("step3: CoCreateInstance OK");
                h
            }
            Err(e) => {
                println!("step3: FAIL CoCreateInstance: {:?}", e);
                return;
            }
        };
    let init: IInitializeWithFile = match handler.cast() {
        Ok(i) => {
            println!("step4: QI IInitializeWithFile OK");
            i
        }
        Err(e) => {
            println!("step4: FAIL QI: {:?}", e);
            return;
        }
    };
    match unsafe { init.Initialize(&HSTRING::from(path.as_str()), 0) } {
        Ok(()) => println!("step5: Initialize OK"),
        Err(e) => {
            println!("step5: FAIL Initialize: {:?}", e);
            return;
        }
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("probe"),
            WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0),
            0,
            0,
            800,
            600,
            None,
            None,
            None,
            None,
        )
    };
    let hwnd = match hwnd {
        Ok(h) => {
            println!("step6: HWND OK");
            h
        }
        Err(e) => {
            println!("step6: FAIL CreateWindow: {:?}", e);
            return;
        }
    };
    let rect = RECT { left: 0, top: 0, right: 800, bottom: 600 };
    match unsafe { handler.SetWindow(hwnd, &rect) } {
        Ok(()) => println!("step7: SetWindow OK"),
        Err(e) => {
            println!("step7: FAIL SetWindow: {:?}", e);
            return;
        }
    }
    match unsafe { handler.DoPreview() } {
        Ok(()) => println!("step8: DoPreview OK — handler works on this machine"),
        Err(e) => println!("step8: FAIL DoPreview: {:?}", e),
    }
    let _ = hwnd;
}
