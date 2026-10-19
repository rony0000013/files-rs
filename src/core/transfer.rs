//! Byte-level file transfer engine (copy / move / delete) with progress,
//! speed, ETA and cancellation. Workers run on async tasks and report into
//! `ProgressEngine`; the StatusCenter renders jobs live.

use crate::core::progress::{OpKind, ProgressEngine};
use anyhow::Context;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Chunk size for streaming copies (256 KiB keeps UI updates smooth).
const CHUNK: usize = 256 * 1024;
/// Minimum interval between progress broadcasts (ms) — avoids render storms.
const REPORT_MS: u128 = 200;

pub enum TransferOutcome {
    Done,
    Cancelled,
    Failed(String),
}

/// Walk `paths`, counting files + total bytes (dirs traversed, links skipped).
pub fn scan_tree(paths: &[PathBuf]) -> (u64, u64) {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut stack: Vec<PathBuf> = paths.to_vec();
    while let Some(p) = stack.pop() {
        let Ok(md) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                for e in rd.flatten() {
                    stack.push(e.path());
                }
            }
        } else if md.is_file() {
            files += 1;
            bytes += md.len();
        }
    }
    (files, bytes)
}

/// Unique destination when `dst` exists: "name (2).ext", "name (3).ext", ...
pub fn unique_dest(mut dst: PathBuf) -> PathBuf {
    if !dst.exists() {
        return dst;
    }
    let stem = dst
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = dst
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = dst.parent().map(|p| p.to_path_buf());
    for i in 2u32..10000 {
        let name = format!("{} ({}){}", stem, i, ext);
        let cand = match &parent {
            Some(p) => p.join(&name),
            None => PathBuf::from(&name),
        };
        if !cand.exists() {
            return cand;
        }
    }
    dst.set_file_name(format!("{} (copy){}", stem, ext));
    dst
}

struct Reporter<'a> {
    engine: &'a ProgressEngine,
    job: uuid::Uuid,
    total: u64,
    done: u64,
    last_report: Instant,
    file: String,
}

impl<'a> Reporter<'a> {
    async fn tick(&mut self, bytes: u64) {
        self.done += bytes;
        let now = Instant::now();
        if now.duration_since(self.last_report).as_millis() >= REPORT_MS {
            self.last_report = now;
            self.engine
                .report(self.job, self.done, self.total, &self.file)
                .await;
        }
    }

    async fn file_done(&mut self) {
        self.last_report = Instant::now();
        self.engine
            .report(self.job, self.done, self.total, &self.file)
            .await;
    }
}

fn cancelled(
    engine: &ProgressEngine,
    job: uuid::Uuid,
) -> bool {
    engine
        .cancel_flag(job)
        .load(Ordering::SeqCst)
}

/// Stream-copy one file with progress + cancellation.
async fn copy_file(
    src: &Path,
    dst: &Path,
    engine: &ProgressEngine,
    job: uuid::Uuid,
    rep: &mut Reporter<'_>,
) -> anyhow::Result<()> {
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let mut rd = tokio::fs::File::open(src)
        .await
        .with_context(|| format!("open {}", src.display()))?;
    let mut wr = tokio::fs::File::create(dst)
        .await
        .with_context(|| format!("create {}", dst.display()))?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        if cancelled(engine, job) {
            anyhow::bail!("cancelled");
        }
        let n = rd.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        wr.write_all(&buf[..n]).await?;
        rep.tick(n as u64).await;
    }
    wr.flush().await?;
    Ok(())
}

/// Iterative deep copy (explicit stack — no recursion, no boxing).
async fn copy_tree_stack(
    roots: Vec<(PathBuf, PathBuf)>,
    engine: &ProgressEngine,
    job: uuid::Uuid,
    rep: &mut Reporter<'_>,
    errors: &mut Vec<String>,
) {
    let mut stack = roots;
    while let Some((src, dst)) = stack.pop() {
        if cancelled(engine, job) {
            return;
        }
        let Ok(md) = tokio::fs::symlink_metadata(&src).await else {
            continue;
        };
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            let _ = tokio::fs::create_dir_all(&dst).await;
            let Ok(mut rd) = tokio::fs::read_dir(&src).await else {
                errors.push(format!("cannot list {}", src.display()));
                continue;
            };
            while let Ok(Some(e)) = rd.next_entry().await {
                stack.push((e.path(), dst.join(e.file_name())));
            }
        } else if md.is_file() {
            rep.file = src
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let dst = unique_dest(dst);
            match copy_file(&src, &dst, engine, job, rep).await {
                Ok(_) => {}
                Err(e) if e.to_string() == "cancelled" => return,
                Err(e) => errors.push(format!("{}: {:#}", src.display(), e)),
            }
            rep.file_done().await;
        }
    }
}

/// Copy `sources` into `dest_dir` with a live job. Same-volume fast path is
/// handled by the caller (move); this always streams bytes.
pub async fn run_copy_job(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> TransferOutcome {
    let (_files, total) = scan_tree(&sources);
    engine.report(job, 0, total.max(1), "").await;
    let mut rep = Reporter {
        engine,
        job,
        total: total.max(1),
        done: 0,
        last_report: Instant::now(),
        file: String::new(),
    };
    let mut errors = Vec::new();
    for src in &sources {
        if cancelled(engine, job) {
            return TransferOutcome::Cancelled;
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string());
        let dst = unique_dest(dest_dir.join(&name));
        copy_tree_stack(vec![(src.clone(), dst)], engine, job, &mut rep, &mut errors).await;
        if cancelled(engine, job) {
            return TransferOutcome::Cancelled;
        }
    }
    engine.report(job, rep.total, rep.total, "").await;
    if errors.is_empty() {
        TransferOutcome::Done
    } else {
        TransferOutcome::Failed(format!(
            "{} error{} (first: {})",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors[0]
        ))
    }
}

/// Move `sources` into `dest_dir`: same-volume rename, else copy + delete.
pub async fn run_move_job(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> TransferOutcome {
    let (_files, total) = scan_tree(&sources);
    let total = total.max(1);
    engine.report(job, 0, total, "").await;
    let mut rep = Reporter {
        engine,
        job,
        total,
        done: 0,
        last_report: Instant::now(),
        file: String::new(),
    };
    let mut errors = Vec::new();
    for src in &sources {
        if cancelled(engine, job) {
            return TransferOutcome::Cancelled;
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string());
        // Size upfront (rename consumes the source before we could measure).
        let (fcount, fbytes) = scan_tree(std::slice::from_ref(src));
        let _ = fcount;
        let dst = unique_dest(dest_dir.join(&name));
        match tokio::fs::rename(src, &dst).await {
            Ok(_) => {
                rep.file = name.clone();
                rep.done = rep.done.saturating_add(fbytes).min(rep.total);
                rep.file_done().await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                // Cross-volume: stream the bytes, then remove the source.
                let dst = unique_dest(dest_dir.join(&name));
                copy_tree_stack(vec![(src.clone(), dst)], engine, job, &mut rep, &mut errors).await;
                if cancelled(engine, job) {
                    return TransferOutcome::Cancelled;
                }
                if src.is_dir() {
                    let _ = tokio::fs::remove_dir_all(src).await;
                } else {
                    let _ = tokio::fs::remove_file(src).await;
                }
            }
            Err(e) => {
                errors.push(format!("{}: {e}", src.display()));
            }
        }
        if cancelled(engine, job) {
            return TransferOutcome::Cancelled;
        }
    }
    engine.report(job, rep.total, rep.total, "").await;
    if errors.is_empty() {
        TransferOutcome::Done
    } else {
        TransferOutcome::Failed(format!(
            "{} error{} (first: {})",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors[0]
        ))
    }
}

/// Delete paths (trash or permanent) with per-item progress.
pub async fn run_delete_job(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    paths: Vec<PathBuf>,
    use_trash: bool,
) -> TransferOutcome {
    let total = paths.len().max(1) as u64;
    let mut done = 0u64;
    let mut errors = Vec::new();
    for p in &paths {
        if cancelled(engine, job) {
            return TransferOutcome::Cancelled;
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        engine.report(job, done, total, &name).await;
        let r = if use_trash {
            trash::delete(p).map_err(|e| anyhow::anyhow!("{e}"))
        } else if p.is_dir() {
            std::fs::remove_dir_all(p).map_err(anyhow::Error::from)
        } else {
            std::fs::remove_file(p).map_err(anyhow::Error::from)
        };
        if let Err(e) = r {
            errors.push(format!("{}: {:#}", p.display(), e));
        }
        done += 1;
        engine.report(job, done, total, &name).await;
    }
    if errors.is_empty() {
        TransferOutcome::Done
    } else {
        TransferOutcome::Failed(format!(
            "{} error{} (first: {})",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors[0]
        ))
    }
}
