//! Byte-level file transfer engine (copy / move / delete) with progress,
//! speed, ETA, pause/resume, and cancellation. Workers run on async tasks and
//! report into `ProgressEngine`; the StatusCenter renders jobs live.

use crate::core::progress::ProgressEngine;
use anyhow::Context;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Process-wide tokio runtime for transfer I/O.
///
/// GPUI executors provide no tokio runtime context, and every `tokio::fs` /
/// `tokio::time` call panics outside one ("there is no reactor running") —
/// which used to abort the whole app on the first paste. Transfer jobs run
/// on this runtime; UI code `await`s the JoinHandle from GPUI executors.
static TOKIO_RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

pub fn io_handle() -> tokio::runtime::Handle {
    TOKIO_RT
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .thread_name("files-rs-io")
                .build()
                .expect("tokio runtime for file transfers")
        })
        .handle()
        .clone()
}

async fn join_transfer_recorded<T: Default>(
    handle: tokio::task::JoinHandle<(TransferOutcome, T)>,
) -> (TransferOutcome, T) {
    match handle.await {
        Ok(res) => res,
        Err(e) => (
            TransferOutcome::Failed(format!("transfer task failed: {e}")),
            T::default(),
        ),
    }
}

/// Copy on the I/O runtime, returning both [`TransferOutcome`] and the `(src, dst)` pairs created.
pub async fn run_copy_job_recorded_rt(
    engine: std::sync::Arc<ProgressEngine>,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> (TransferOutcome, Vec<(PathBuf, PathBuf)>) {
    let handle = io_handle();
    join_transfer_recorded(
        handle.spawn(async move { run_copy_job_recorded(&engine, job, sources, dest_dir).await }),
    )
    .await
}

/// Copy on the I/O runtime (GPUI-safe wrapper around [`run_copy_job`]).
pub async fn run_copy_job_rt(
    engine: std::sync::Arc<ProgressEngine>,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> TransferOutcome {
    run_copy_job_recorded_rt(engine, job, sources, dest_dir).await.0
}

/// Move on the I/O runtime, returning both [`TransferOutcome`] and the `(src, dst)` pairs moved.
pub async fn run_move_job_recorded_rt(
    engine: std::sync::Arc<ProgressEngine>,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> (TransferOutcome, Vec<(PathBuf, PathBuf)>) {
    let handle = io_handle();
    join_transfer_recorded(
        handle.spawn(async move { run_move_job_recorded(&engine, job, sources, dest_dir).await }),
    )
    .await
}

/// Move on the I/O runtime (GPUI-safe wrapper around [`run_move_job`]).
pub async fn run_move_job_rt(
    engine: std::sync::Arc<ProgressEngine>,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> TransferOutcome {
    run_move_job_recorded_rt(engine, job, sources, dest_dir).await.0
}

/// Delete on the I/O runtime, returning both [`TransferOutcome`] and the paths moved to Trash.
pub async fn run_delete_job_recorded_rt(
    engine: std::sync::Arc<ProgressEngine>,
    job: uuid::Uuid,
    paths: Vec<PathBuf>,
    use_trash: bool,
) -> (TransferOutcome, Vec<PathBuf>) {
    let handle = io_handle();
    join_transfer_recorded(
        handle.spawn(async move { run_delete_job_recorded(&engine, job, paths, use_trash).await }),
    )
    .await
}

/// Delete on the I/O runtime (GPUI-safe wrapper around [`run_delete_job`]).
pub async fn run_delete_job_rt(
    engine: std::sync::Arc<ProgressEngine>,
    job: uuid::Uuid,
    paths: Vec<PathBuf>,
    use_trash: bool,
) -> TransferOutcome {
    run_delete_job_recorded_rt(engine, job, paths, use_trash).await.0
}

/// Chunk size for streaming copies (256 KiB keeps UI updates smooth).
const CHUNK: usize = 256 * 1024;
/// Minimum interval between progress broadcasts (ms) — avoids render storms.
const REPORT_MS: u128 = 150;

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
    total_bytes: u64,
    done_bytes: u64,
    total_items: u64,
    done_items: u64,
    last_report: Instant,
    file: String,
}

impl<'a> Reporter<'a> {
    async fn tick(&mut self, bytes: u64) {
        self.done_bytes += bytes;
        let now = Instant::now();
        if now.duration_since(self.last_report).as_millis() >= REPORT_MS {
            self.last_report = now;
            self.engine
                .report_with_items(
                    self.job,
                    self.done_bytes,
                    self.total_bytes,
                    self.done_items,
                    self.total_items,
                    &self.file,
                )
                .await;
        }
    }

    async fn file_done(&mut self) {
        self.done_items = (self.done_items + 1).min(self.total_items.max(1));
        self.last_report = Instant::now();
        self.engine
            .report_with_items(
                self.job,
                self.done_bytes,
                self.total_bytes,
                self.done_items,
                self.total_items,
                &self.file,
            )
            .await;
    }
}

fn cancelled(engine: &ProgressEngine, job: uuid::Uuid) -> bool {
    engine.cancel_flag(job).load(Ordering::SeqCst)
}

async fn wait_if_paused(engine: &ProgressEngine, job: uuid::Uuid) -> bool {
    let pflag = engine.pause_flag(job);
    let cflag = engine.cancel_flag(job);
    while pflag.load(Ordering::SeqCst) {
        if cflag.load(Ordering::SeqCst) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    cflag.load(Ordering::SeqCst)
}

/// Stream-copy one file with progress, pause/resume, and cancellation.
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
        if wait_if_paused(engine, job).await {
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
        if wait_if_paused(engine, job).await {
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

/// Copy `sources` into `dest_dir` with a live job.
pub async fn run_copy_job(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> TransferOutcome {
    run_copy_job_recorded(engine, job, sources, dest_dir).await.0
}

/// Copy `sources` into `dest_dir` and return `(outcome, created_src_dst_pairs)`.
pub async fn run_copy_job_recorded(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> (TransferOutcome, Vec<(PathBuf, PathBuf)>) {
    let (files, total) = scan_tree(&sources);
    let total_bytes = total.max(1);
    let total_items = files.max(sources.len() as u64).max(1);
    engine
        .report_with_items(job, 0, total_bytes, 0, total_items, "")
        .await;
    let mut rep = Reporter {
        engine,
        job,
        total_bytes,
        done_bytes: 0,
        total_items,
        done_items: 0,
        last_report: Instant::now(),
        file: String::new(),
    };
    let mut errors = Vec::new();
    let mut copied = Vec::new();
    for src in &sources {
        if wait_if_paused(engine, job).await {
            return (TransferOutcome::Cancelled, copied);
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string());
        let dst = unique_dest(dest_dir.join(&name));
        let err_before = errors.len();
        copy_tree_stack(
            vec![(src.clone(), dst.clone())],
            engine,
            job,
            &mut rep,
            &mut errors,
        )
        .await;
        if cancelled(engine, job) {
            return (TransferOutcome::Cancelled, copied);
        }
        if errors.len() == err_before && dst.symlink_metadata().is_ok() {
            copied.push((src.clone(), dst));
        }
    }
    engine
        .report_with_items(job, rep.total_bytes, rep.total_bytes, rep.total_items, rep.total_items, "")
        .await;
    let outcome = if errors.is_empty() {
        TransferOutcome::Done
    } else {
        TransferOutcome::Failed(format!(
            "{} error{} (first: {})",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors[0]
        ))
    };
    (outcome, copied)
}

/// Move `sources` into `dest_dir`: same-volume rename, else copy + delete.
pub async fn run_move_job(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> TransferOutcome {
    run_move_job_recorded(engine, job, sources, dest_dir).await.0
}

/// Move `sources` into `dest_dir` and return `(outcome, moved_src_dst_pairs)`.
pub async fn run_move_job_recorded(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
) -> (TransferOutcome, Vec<(PathBuf, PathBuf)>) {
    let (files, total) = scan_tree(&sources);
    let total_bytes = total.max(1);
    let total_items = files.max(sources.len() as u64).max(1);
    engine
        .report_with_items(job, 0, total_bytes, 0, total_items, "")
        .await;
    let mut rep = Reporter {
        engine,
        job,
        total_bytes,
        done_bytes: 0,
        total_items,
        done_items: 0,
        last_report: Instant::now(),
        file: String::new(),
    };
    let mut errors = Vec::new();
    let mut moved = Vec::new();
    for src in &sources {
        if wait_if_paused(engine, job).await {
            return (TransferOutcome::Cancelled, moved);
        }
        let name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".to_string());
        let (_fcount, fbytes) = scan_tree(std::slice::from_ref(src));
        let dst = unique_dest(dest_dir.join(&name));
        match tokio::fs::rename(src, &dst).await {
            Ok(_) => {
                moved.push((src.clone(), dst));
                rep.file = name.clone();
                rep.done_bytes = rep.done_bytes.saturating_add(fbytes).min(rep.total_bytes);
                rep.file_done().await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                let dst = unique_dest(dest_dir.join(&name));
                let err_before = errors.len();
                copy_tree_stack(
                    vec![(src.clone(), dst.clone())],
                    engine,
                    job,
                    &mut rep,
                    &mut errors,
                )
                .await;
                if cancelled(engine, job) {
                    return (TransferOutcome::Cancelled, moved);
                }
                if errors.len() == err_before {
                    if src.is_dir() {
                        let _ = tokio::fs::remove_dir_all(src).await;
                    } else {
                        let _ = tokio::fs::remove_file(src).await;
                    }
                    if dst.symlink_metadata().is_ok() {
                        moved.push((src.clone(), dst));
                    }
                }
            }
            Err(e) => {
                errors.push(format!("{}: {e}", src.display()));
            }
        }
        if cancelled(engine, job) {
            return (TransferOutcome::Cancelled, moved);
        }
    }
    engine
        .report_with_items(job, rep.total_bytes, rep.total_bytes, rep.total_items, rep.total_items, "")
        .await;
    let outcome = if errors.is_empty() {
        TransferOutcome::Done
    } else {
        TransferOutcome::Failed(format!(
            "{} error{} (first: {})",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors[0]
        ))
    };
    (outcome, moved)
}

/// Delete paths (trash or permanent) with per-item progress and pause/cancel support.
pub async fn run_delete_job(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    paths: Vec<PathBuf>,
    use_trash: bool,
) -> TransferOutcome {
    run_delete_job_recorded(engine, job, paths, use_trash).await.0
}

/// Delete paths and return `(outcome, trashed_paths)` when `use_trash` is true.
pub async fn run_delete_job_recorded(
    engine: &ProgressEngine,
    job: uuid::Uuid,
    paths: Vec<PathBuf>,
    use_trash: bool,
) -> (TransferOutcome, Vec<PathBuf>) {
    let total = paths.len().max(1) as u64;
    let mut done = 0u64;
    let mut errors = Vec::new();
    let mut trashed = Vec::new();
    for p in &paths {
        if wait_if_paused(engine, job).await {
            return (TransferOutcome::Cancelled, trashed);
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        engine
            .report_with_items(job, done, total, done, total, &name)
            .await;
        let r = if use_trash {
            trash::delete(p).map_err(|e| anyhow::anyhow!("{e}"))
        } else if p.is_dir() {
            std::fs::remove_dir_all(p).map_err(anyhow::Error::from)
        } else {
            std::fs::remove_file(p).map_err(anyhow::Error::from)
        };
        match r {
            Ok(()) if use_trash => trashed.push(p.clone()),
            Ok(()) => {}
            Err(e) => errors.push(format!("{}: {:#}", p.display(), e)),
        }
        done += 1;
        engine
            .report_with_items(job, done, total, done, total, &name)
            .await;
    }
    let outcome = if errors.is_empty() {
        TransferOutcome::Done
    } else {
        TransferOutcome::Failed(format!(
            "{} error{} (first: {})",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors[0]
        ))
    };
    (outcome, trashed)
}
