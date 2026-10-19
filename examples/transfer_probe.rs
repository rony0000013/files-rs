//! Headless probe: copy a temp tree through the transfer engine.
//! Usage: cargo run --release --example transfer_probe

use files_rs::core::progress::{JobUpdate, OpKind, ProgressEngine};
use files_rs::core::transfer::run_copy_job;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let base = std::env::temp_dir().join("files_rs_transfer_probe");
    let _ = std::fs::remove_dir_all(&base);
    let src = base.join("src");
    let dst = base.join("dst");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    // ~5 x 1MB files + a small one.
    for i in 0..5 {
        let data = vec![i as u8; 1024 * 1024];
        std::fs::write(src.join(format!("f{}.bin", i)), &data).unwrap();
    }
    std::fs::write(src.join("sub").join("s.txt"), b"hello").unwrap();

    let engine = ProgressEngine::new();
    let mut rx = engine.subscribe();
    let job = engine
        .start_job("Copy (6 items)".to_string(), OpKind::Copy)
        .await;
    let sources = vec![src.clone()];
    let out = run_copy_job(&engine, job, sources, dst.clone()).await;
    println!(
        "OUTCOME={}",
        match out {
            files_rs::core::transfer::TransferOutcome::Done => "done",
            files_rs::core::transfer::TransferOutcome::Cancelled => "cancelled",
            files_rs::core::transfer::TransferOutcome::Failed(e) => {
                println!("FAIL DETAIL: {}", e);
                "failed"
            }
        }
    );
    // Drain updates.
    let mut n_updates = 0u32;
    let mut last_pct = 0.0f32;
    while let Ok(u) = rx.try_recv() {
        if let JobUpdate::UpdateStatus(_, st) = u {
            n_updates += 1;
            if let files_rs::core::progress::OperationStatus::InProgress {
                progress, ..
            } = st
            {
                last_pct = progress;
            }
        }
    }
    println!("UPDATES={} LAST_PCT={:.2}", n_updates, last_pct);
    // Verify bytes on disk.
    let mut bytes = 0u64;
    let mut stack = vec![dst.clone()];
    while let Some(p) = stack.pop() {
        for e in std::fs::read_dir(&p).unwrap().flatten() {
            let pp = e.path();
            if pp.is_dir() {
                stack.push(pp);
            } else {
                bytes += std::fs::metadata(&pp).unwrap().len();
            }
        }
    }
    println!("DST_BYTES={} EXPECTED={}", bytes, 5 * 1024 * 1024 + 5);
    let _ = PathBuf::from("x");
    let _ = std::fs::remove_dir_all(&base);
}
