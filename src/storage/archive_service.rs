use crate::core::path_util::FilesPath;
use crate::domain::metadata::FileItem;
use crate::storage::storage::FileType;
use anyhow::{anyhow, bail, Result};
use bzip2::read::MultiBzDecoder;
use flate2::read::MultiGzDecoder;
use lru::LruCache;
use lzma_rust2::{LzmaReader, XzReader};
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;
use zip::ZipWriter;

/// Supported archive and compressed stream formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    TarLzma,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
}

impl ArchiveFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Zip => "ZIP Archive",
            Self::SevenZ => "7-Zip Archive",
            Self::Tar => "TAR Archive",
            Self::TarGz => "GZip Tar Archive (.tar.gz)",
            Self::TarBz2 => "BZip2 Tar Archive (.tar.bz2)",
            Self::TarXz => "XZ Tar Archive (.tar.xz)",
            Self::TarZst => "Zstd Tar Archive (.tar.zst)",
            Self::TarLzma => "LZMA Tar Archive (.tar.lzma)",
            Self::Gzip => "GZip Compressed File",
            Self::Bzip2 => "BZip2 Compressed File",
            Self::Xz => "XZ Compressed File",
            Self::Zstd => "Zstd Compressed File",
        }
    }

    pub fn short_badge(&self) -> &'static str {
        match self {
            Self::Zip => "ZIP",
            Self::SevenZ => "7Z",
            Self::Tar => "TAR",
            Self::TarGz => "TAR.GZ",
            Self::TarBz2 => "TAR.BZ2",
            Self::TarXz => "TAR.XZ",
            Self::TarZst => "TAR.ZST",
            Self::TarLzma => "TAR.LZMA",
            Self::Gzip => "GZ",
            Self::Bzip2 => "BZ2",
            Self::Xz => "XZ",
            Self::Zstd => "ZST",
        }
    }

    pub fn detect(path: &Path) -> Option<Self> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_ascii_lowercase())?;

        // Compound TAR extensions first
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Some(Self::TarGz);
        }
        if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
            return Some(Self::TarBz2);
        }
        if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            return Some(Self::TarXz);
        }
        if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
            return Some(Self::TarZst);
        }
        if name.ends_with(".tar.lzma") || name.ends_with(".tlz") {
            return Some(Self::TarLzma);
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())?;

        match ext.as_str() {
            "zip" | "zipx" | "jar" | "war" | "apk" | "aab" | "ipa" | "whl" | "nupkg"
            | "vsix" | "xpi" | "crx" | "cbz" | "epub" | "msix" | "appx" => Some(Self::Zip),
            "7z" => Some(Self::SevenZ),
            "tar" => Some(Self::Tar),
            "gz" | "gzip" => Some(Self::Gzip),
            "bz2" => Some(Self::Bzip2),
            "xz" => Some(Self::Xz),
            "zst" | "zstd" => Some(Self::Zstd),
            _ => None,
        }
    }

    /// Suggested base folder name when extracting into a subfolder (strips `.tar.gz`, etc.).
    pub fn suggested_folder_name(path: &Path) -> String {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("extracted");
        let lower = name.to_ascii_lowercase();
        for compound in [
            ".tar.gz",
            ".tar.bz2",
            ".tar.xz",
            ".tar.zst",
            ".tar.lzma",
        ] {
            if lower.ends_with(compound) && name.len() > compound.len() {
                return name[..name.len() - compound.len()].to_string();
            }
        }
        path.file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("extracted")
            .to_string()
    }
}

/// Hard safety limits to defeat zip bombs, decompression loops, path traversal,
/// and excessive memory usage on malformed archives.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveSafetyPolicy {
    pub max_entries: usize,
    pub max_total_uncompressed_bytes: u64,
    pub max_single_file_bytes: u64,
    pub max_compression_ratio: f64,
    pub ratio_grace_bytes: u64,
    pub max_path_depth: usize,
    pub lzma_mem_limit_kb: u32,
}

impl Default for ArchiveSafetyPolicy {
    fn default() -> Self {
        Self {
            max_entries: 200_000,
            max_total_uncompressed_bytes: 8 * 1024 * 1024 * 1024, // 8 GiB
            max_single_file_bytes: 4 * 1024 * 1024 * 1024,        // 4 GiB
            max_compression_ratio: 100.0,
            ratio_grace_bytes: 10 * 1024 * 1024, // 10 MiB grace window
            max_path_depth: 64,
            lzma_mem_limit_kb: 256 * 1024, // 256 MiB dictionary limit
        }
    }
}

/// Detailed metadata for one entry inside an archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntryInfo {
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed_size: u64,
    pub modified: Option<SystemTime>,
    pub is_encrypted: bool,
}

/// Full inventory returned by scanning/listing an archive.
#[derive(Debug, Clone)]
pub struct ArchiveInventory {
    pub format: ArchiveFormat,
    pub compressed_size: u64,
    pub total_uncompressed: u64,
    pub entries: Arc<Vec<ArchiveEntryInfo>>,
    pub is_encrypted: bool,
}

/// User-configurable extraction job parameters.
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub archive_path: PathBuf,
    pub destination: PathBuf,
    pub selected_entries: Option<HashSet<String>>,
    pub create_subfolder: bool,
    pub overwrite: bool,
    pub password: Option<String>,
    pub policy: ArchiveSafetyPolicy,
}

/// Progress snapshot emitted during extraction.
#[derive(Debug, Clone)]
pub struct ExtractProgress {
    pub current_item: String,
    pub entries_done: usize,
    pub entries_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// Result summary from an extraction job.
#[derive(Debug, Clone)]
pub struct ExtractReport {
    pub final_destination: PathBuf,
    pub extracted_files: usize,
    pub extracted_bytes: u64,
}

#[derive(Hash, PartialEq, Eq)]
struct ArchiveIndexCacheKey {
    path: PathBuf,
    size: u64,
    mtime_secs: u64,
}

static ARCHIVE_INDEX_CACHE: OnceLock<Mutex<LruCache<ArchiveIndexCacheKey, ArchiveInventory>>> =
    OnceLock::new();

fn index_cache() -> &'static Mutex<LruCache<ArchiveIndexCacheKey, ArchiveInventory>> {
    ARCHIVE_INDEX_CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(NonZeroUsize::new(32).expect("nonzero cache cap")))
    })
}

/// Wraps the compressed input reader to measure exact compressed bytes consumed.
struct CountingReader<R> {
    inner: R,
    compressed_read: Arc<AtomicU64>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.compressed_read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Wraps a decompressing reader and enforces zip-bomb byte, ratio, and cancellation caps
/// on *actual* decompressed output (ignoring untrusted header sizes).
struct BoundedDecompressReader<'a, R> {
    inner: R,
    entry_read: u64,
    total_read: &'a AtomicU64,
    compressed_read: Arc<AtomicU64>,
    entry_limit: u64,
    policy: ArchiveSafetyPolicy,
    cancel: &'a AtomicBool,
}

impl<'a, R: Read> Read for BoundedDecompressReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Archive operation cancelled",
            ));
        }
        let n = self.inner.read(buf)?;
        if n == 0 {
            return Ok(0);
        }
        let n_u64 = n as u64;
        self.entry_read = self.entry_read.saturating_add(n_u64);
        if self.entry_read > self.entry_limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Safety limit exceeded: entry decompressed output surpassed {} bytes",
                    self.entry_limit
                ),
            ));
        }
        let total = self.total_read.fetch_add(n_u64, Ordering::Relaxed) + n_u64;
        if total > self.policy.max_total_uncompressed_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Zip bomb protection: total uncompressed output exceeded {} GiB limit",
                    self.policy.max_total_uncompressed_bytes / (1024 * 1024 * 1024)
                ),
            ));
        }
        if total > self.policy.ratio_grace_bytes {
            let compressed = self.compressed_read.load(Ordering::Relaxed).max(1);
            if (total as f64) > (compressed as f64) * self.policy.max_compression_ratio {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Zip bomb protection triggered: decompression ratio exceeded {:.0}:1 ({} bytes from {} compressed bytes)",
                        self.policy.max_compression_ratio, total, compressed
                    ),
                ));
            }
        }
        Ok(n)
    }
}

/// Normalizes an archive relative path and rejects traversal / reserved OS names.
pub fn sanitize_archive_rel_path(raw: &str, policy: &ArchiveSafetyPolicy) -> Result<String> {
    if raw.contains('\0') {
        bail!("Invalid archive entry path containing NUL byte");
    }
    let normalized = raw.replace('\\', "/");
    let trimmed = normalized.trim_start_matches('/');
    // Strip leading `./` segments
    let mut parts: Vec<&str> = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            bail!("Path traversal ('..') detected in archive entry '{}'", raw);
        }
        if parts.is_empty() && seg.len() >= 2 && seg.as_bytes()[1] == b':' {
            bail!("Drive prefix rejected in archive entry '{}'", raw);
        }
        if is_windows_reserved_name(seg) {
            bail!("Reserved filename '{}' rejected in archive entry '{}'", seg, raw);
        }
        parts.push(seg);
        if parts.len() > policy.max_path_depth {
            bail!(
                "Archive path depth exceeds {} segments in '{}'",
                policy.max_path_depth,
                raw
            );
        }
    }
    Ok(parts.join("/"))
}

fn is_windows_reserved_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment).trim_end();
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn ensure_within_root(dest_root: &Path, candidate: &Path) -> Result<()> {
    let canonical_root = std::fs::canonicalize(dest_root).unwrap_or_else(|_| dest_root.to_path_buf());
    // Canonicalize deepest existing ancestor so non-existent child targets can be validated
    let mut probe = candidate.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !probe.exists() {
        if let Some(name) = probe.file_name() {
            tail.push(name.to_os_string());
        } else {
            break;
        }
        if !probe.pop() {
            break;
        }
    }
    let mut resolved = std::fs::canonicalize(&probe).unwrap_or(probe);
    for seg in tail.into_iter().rev() {
        resolved.push(seg);
    }
    if !resolved.starts_with(&canonical_root) {
        bail!(
            "Zip slip detected: '{}' resolves outside '{}'",
            resolved.display(),
            canonical_root.display()
        );
    }
    Ok(())
}

/// Pure-Rust TAR entry header parsed from 512-byte USTAR/PAX/GNU blocks.
struct ParsedTarEntry {
    rel_path: String,
    is_dir: bool,
    is_regular_file: bool,
    size: u64,
    mtime: Option<SystemTime>,
}

fn parse_tar_number(field: &[u8]) -> u64 {
    if !field.is_empty() && (field[0] & 0x80) != 0 {
        // GNU base-256 big-endian
        let mut val = u64::from(field[0] & 0x7F);
        for &b in &field[1..] {
            val = (val << 8) | u64::from(b);
        }
        return val;
    }
    let s = String::from_utf8_lossy(field);
    let trimmed = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if trimmed.is_empty() {
        0
    } else {
        u64::from_str_radix(trimmed, 8).unwrap_or(0)
    }
}

fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = reader.read(&mut buf[filled..])?;
        if n == 0 {
            if filled == 0 {
                return Ok(false);
            }
            bail!("Unexpected EOF inside TAR stream");
        }
        filled += n;
    }
    Ok(true)
}

fn parse_pax_attributes(data: &[u8]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut pos = 0;
    while pos < data.len() {
        let rest = &data[pos..];
        let Some(sp) = rest.iter().position(|&b| b == b' ') else {
            break;
        };
        let len_str = std::str::from_utf8(&rest[..sp]).unwrap_or("0");
        let rec_len: usize = len_str.parse().unwrap_or(0);
        if rec_len == 0 || pos + rec_len > data.len() || rec_len <= sp + 1 {
            break;
        }
        let kv = &rest[sp + 1..rec_len];
        let kv_str = String::from_utf8_lossy(kv);
        let kv_trimmed = kv_str.trim_end_matches('\n');
        if let Some((k, v)) = kv_trimmed.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
        pos += rec_len;
    }
    map
}

fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn visit_tar_stream<R: Read, F>(
    reader: &mut R,
    policy: &ArchiveSafetyPolicy,
    cancel: &AtomicBool,
    mut on_entry: F,
) -> Result<()>
where
    F: FnMut(ParsedTarEntry, &mut dyn Read) -> Result<bool>,
{
    let mut header = [0u8; 512];
    let mut pax_global: BTreeMap<String, String> = BTreeMap::new();
    let mut pax_next: Option<BTreeMap<String, String>> = None;
    let mut gnu_longname: Option<String> = None;
    let mut seen_entries = 0usize;

    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Archive operation cancelled");
        }
        if !read_exact_or_eof(reader, &mut header)? {
            break;
        }
        if header.iter().all(|&b| b == 0) {
            break;
        }

        let typeflag = header[156];
        let mut size = parse_tar_number(&header[124..136]);

        // Handle metadata pseudo-headers first
        if typeflag == b'L' || typeflag == b'x' || typeflag == b'g' || typeflag == b'K' {
            if size > 16 * 1024 * 1024 {
                bail!("TAR metadata header too large ({} bytes)", size);
            }
            let mut meta = vec![0u8; size as usize];
            if size > 0 {
                reader.read_exact(&mut meta)?;
            }
            let pad = (512 - (size % 512)) % 512;
            if pad > 0 {
                let mut skip = [0u8; 512];
                reader.read_exact(&mut skip[..pad as usize])?;
            }
            match typeflag {
                b'L' => {
                    gnu_longname = Some(cstr(&meta));
                }
                b'x' => {
                    pax_next = Some(parse_pax_attributes(&meta));
                }
                b'g' => {
                    for (k, v) in parse_pax_attributes(&meta) {
                        pax_global.insert(k, v);
                    }
                }
                _ => {}
            }
            continue;
        }

        seen_entries += 1;
        if seen_entries > policy.max_entries {
            bail!(
                "Archive entry count exceeds safety cap ({})",
                policy.max_entries
            );
        }

        let pax = pax_next.take();
        if let Some(ref p) = pax {
            if let Some(s) = p.get("size").and_then(|v| v.parse::<u64>().ok()) {
                size = s;
            }
        } else if let Some(s) = pax_global.get("size").and_then(|v| v.parse::<u64>().ok()) {
            if size == 0 {
                size = s;
            }
        }

        let raw_name = if let Some(p_name) = pax.as_ref().and_then(|p| p.get("path")).cloned() {
            gnu_longname = None;
            p_name
        } else if let Some(long) = gnu_longname.take() {
            long
        } else {
            let name_part = cstr(&header[0..100]);
            let prefix_part = cstr(&header[345..500]);
            if prefix_part.is_empty() {
                name_part
            } else {
                format!("{}/{}", prefix_part, name_part)
            }
        };

        let mtime_secs = parse_tar_number(&header[136..148]);
        let mtime = if mtime_secs > 0 {
            Some(UNIX_EPOCH + Duration::from_secs(mtime_secs))
        } else {
            None
        };

        let is_dir = typeflag == b'5' || raw_name.ends_with('/');
        // Skip symlinks (2), hardlinks (1), char/block/fifo devices for security
        let is_regular_file = matches!(typeflag, b'0' | 0 | b'7') && !is_dir;

        let rel_path = match sanitize_archive_rel_path(&raw_name, policy) {
            Ok(p) => p,
            Err(err) => {
                // Only reject on traversal if caller doesn't skip; actually fail loudly on Zip Slip!
                return Err(err);
            }
        };

        let pad = (512 - (size % 512)) % 512;
        if rel_path.is_empty() || (!is_dir && !is_regular_file) {
            // Drain entry payload safely without exposing symlink/device
            let mut take_r = reader.take(size);
            std::io::copy(&mut take_r, &mut std::io::sink())?;
            if pad > 0 {
                let mut skip = [0u8; 512];
                reader.read_exact(&mut skip[..pad as usize])?;
            }
            continue;
        }

        if size > policy.max_single_file_bytes {
            bail!(
                "Archive entry '{}' size ({} bytes) exceeds {} GiB limit",
                rel_path,
                size,
                policy.max_single_file_bytes / (1024 * 1024 * 1024)
            );
        }

        let entry = ParsedTarEntry {
            rel_path,
            is_dir,
            is_regular_file,
            size,
            mtime,
        };

        let mut payload_reader = reader.take(size);
        let cont = on_entry(entry, &mut payload_reader)?;
        // Drain any unread payload bytes before skipping padding so alignment stays intact
        if payload_reader.limit() > 0 {
            std::io::copy(&mut payload_reader, &mut std::io::sink())?;
        }
        if pad > 0 {
            let mut skip = [0u8; 512];
            reader.read_exact(&mut skip[..pad as usize])?;
        }
        if !cont {
            break;
        }
    }

    Ok(())
}

fn open_decompressed_stream<'a>(
    format: ArchiveFormat,
    path: &Path,
    compressed_read: Arc<AtomicU64>,
    total_read: &'a AtomicU64,
    policy: ArchiveSafetyPolicy,
    cancel: &'a AtomicBool,
) -> Result<Box<dyn Read + 'a>> {
    let file = File::open(path)?;
    let buf = BufReader::with_capacity(64 * 1024, file);
    let counting = CountingReader {
        inner: buf,
        compressed_read: compressed_read.clone(),
    };
    let raw_stream: Box<dyn Read> = match format {
        ArchiveFormat::Tar => Box::new(counting),
        ArchiveFormat::TarGz | ArchiveFormat::Gzip => Box::new(MultiGzDecoder::new(counting)),
        ArchiveFormat::TarBz2 | ArchiveFormat::Bzip2 => Box::new(MultiBzDecoder::new(counting)),
        ArchiveFormat::TarXz | ArchiveFormat::Xz => {
            Box::new(XzReader::new_mem_limit(counting, true, policy.lzma_mem_limit_kb))
        }
        ArchiveFormat::TarZst | ArchiveFormat::Zstd => Box::new(zstd::stream::read::Decoder::new(counting)?),
        ArchiveFormat::TarLzma => Box::new(
            LzmaReader::new_mem_limit(counting, policy.lzma_mem_limit_kb, None)
                .map_err(|e| anyhow!("LZMA header error: {}", e))?,
        ),
        _ => bail!("Not a stream-compressed archive format"),
    };
    let bounded = BoundedDecompressReader {
        inner: raw_stream,
        entry_read: 0,
        total_read,
        compressed_read,
        entry_limit: policy.max_total_uncompressed_bytes,
        policy,
        cancel,
    };
    Ok(Box::new(bounded))
}

fn copy_bounded_entry<R: Read, W: Write>(
    src: &mut R,
    dst: &mut W,
    entry_limit: u64,
    total_read: &AtomicU64,
    compressed_read: &AtomicU64,
    policy: &ArchiveSafetyPolicy,
    cancel: &AtomicBool,
) -> Result<u64> {
    let mut reader = BoundedDecompressReader {
        inner: src,
        entry_read: 0,
        total_read,
        compressed_read: Arc::new(AtomicU64::new(compressed_read.load(Ordering::Relaxed))),
        entry_limit,
        policy: *policy,
        cancel,
    };
    let mut buf = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Archive operation cancelled");
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
    }
    Ok(written)
}

fn single_stream_inner_name(path: &Path, format: ArchiveFormat) -> String {
    if format == ArchiveFormat::Gzip {
        if let Ok(file) = File::open(path) {
            let dec = MultiGzDecoder::new(BufReader::new(file));
            if let Some(hdr) = dec.header() {
                if let Some(fname) = hdr.filename() {
                    let s = String::from_utf8_lossy(fname).trim().to_string();
                    if let Ok(clean) = sanitize_archive_rel_path(&s, &ArchiveSafetyPolicy::default())
                    {
                        if let Some(base) = clean.rsplit('/').next() {
                            if !base.is_empty() {
                                return base.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output");
    let lower = name.to_ascii_lowercase();
    for suffix in [".gzip", ".gz", ".bz2", ".xz", ".zstd", ".zst"] {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    format!("{}.out", name)
}

fn dos_datetime_to_system_time(dt: Option<zip::DateTime>) -> Option<SystemTime> {
    let dt = dt?;
    let date = chrono::NaiveDate::from_ymd_opt(dt.year() as i32, dt.month() as u32, dt.day() as u32)?;
    let time = chrono::NaiveTime::from_hms_opt(dt.hour() as u32, dt.minute() as u32, dt.second() as u32)?;
    let ndt = date.and_time(time);
    let secs = ndt.and_utc().timestamp();
    if secs >= 0 {
        Some(UNIX_EPOCH + Duration::from_secs(secs as u64))
    } else {
        None
    }
}

fn filetime_to_system_time(ft: sevenz_rust::nt_time::FileTime) -> Option<SystemTime> {
    let unix_secs = i64::try_from(ft.to_unix_time()).ok()?;
    if unix_secs >= 0 {
        Some(UNIX_EPOCH + Duration::from_secs(unix_secs as u64))
    } else {
        None
    }
}

fn normalize_virtual_dir(dir: &str) -> String {
    dir.replace('\\', "/")
        .trim_matches('/')
        .to_string()
}

pub struct ArchiveService;

impl ArchiveService {
    pub fn detect_format(path: &Path) -> Option<ArchiveFormat> {
        ArchiveFormat::detect(path)
    }

    pub fn is_supported_archive(path: &Path) -> bool {
        ArchiveFormat::detect(path).is_some()
    }

    pub fn is_browsable_archive(path: &Path) -> bool {
        ArchiveFormat::detect(path).is_some()
    }

    /// Splits a virtual archive path like `C:\dir\foo.zip\sub\bar` into `(C:\dir\foo.zip, "sub/bar")`.
    /// Returns `Some((archive_path, ""))` when `path` itself is an existing browsable archive file.
    pub fn split_archive_path(path: &Path) -> Option<(PathBuf, String)> {
        if path.is_dir() {
            return None;
        }
        for ancestor in path.ancestors() {
            if ancestor.as_os_str().is_empty() {
                break;
            }
            if Self::is_browsable_archive(ancestor) && ancestor.is_file() {
                let rel = path
                    .strip_prefix(ancestor)
                    .ok()
                    .map(|p| {
                        p.components()
                            .filter_map(|c| match c {
                                std::path::Component::Normal(os) => {
                                    Some(os.to_string_lossy().to_string())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("/")
                    })
                    .unwrap_or_default();
                return Some((ancestor.to_path_buf(), rel));
            }
        }
        None
    }

    /// Lists detailed entries across all supported archive formats, with bounded LRU caching
    /// and zip-bomb header/stream validation.
    pub fn list_entries_detailed(
        archive_path: &Path,
        password: Option<&str>,
    ) -> Result<ArchiveInventory> {
        let format = ArchiveFormat::detect(archive_path)
            .ok_or_else(|| anyhow!("Unsupported archive format: {}", archive_path.display()))?;
        let meta = std::fs::metadata(archive_path)?;
        let compressed_size = meta.len();
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let cache_key = ArchiveIndexCacheKey {
            path: archive_path.to_path_buf(),
            size: compressed_size,
            mtime_secs,
        };

        // Only serve from cache when no custom password was supplied (or unencrypted)
        if password.is_none() {
            if let Ok(mut cache) = index_cache().lock() {
                if let Some(hit) = cache.get(&cache_key) {
                    return Ok(hit.clone());
                }
            }
        }

        let policy = ArchiveSafetyPolicy::default();
        let cancel = AtomicBool::new(false);
        let inventory = match format {
            ArchiveFormat::Zip => Self::list_zip_entries(archive_path, compressed_size, &policy)?,
            ArchiveFormat::SevenZ => {
                Self::list_7z_entries(archive_path, compressed_size, password, &policy)?
            }
            ArchiveFormat::Tar
            | ArchiveFormat::TarGz
            | ArchiveFormat::TarBz2
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarZst
            | ArchiveFormat::TarLzma => {
                Self::list_tar_entries(archive_path, format, compressed_size, &policy, &cancel)?
            }
            ArchiveFormat::Gzip
            | ArchiveFormat::Bzip2
            | ArchiveFormat::Xz
            | ArchiveFormat::Zstd => {
                let inner_name = single_stream_inner_name(archive_path, format);
                let modified = meta.modified().ok();
                let entry = ArchiveEntryInfo {
                    name: inner_name.clone(),
                    relative_path: inner_name,
                    is_dir: false,
                    size: compressed_size,
                    compressed_size,
                    modified,
                    is_encrypted: false,
                };
                ArchiveInventory {
                    format,
                    compressed_size,
                    total_uncompressed: compressed_size,
                    entries: Arc::new(vec![entry]),
                    is_encrypted: false,
                }
            }
        };

        if let Ok(mut cache) = index_cache().lock() {
            cache.put(cache_key, inventory.clone());
        }
        Ok(inventory)
    }

    fn list_zip_entries(
        archive_path: &Path,
        compressed_size: u64,
        policy: &ArchiveSafetyPolicy,
    ) -> Result<ArchiveInventory> {
        let file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
        let len = archive.len();
        if len > policy.max_entries {
            bail!(
                "ZIP archive contains {} entries, exceeding safety limit {}",
                len,
                policy.max_entries
            );
        }

        let mut entries = Vec::with_capacity(len);
        let mut total_uncompressed = 0u64;
        let mut any_encrypted = false;

        for i in 0..len {
            let entry = archive.by_index_raw(i)?;
            let raw_name = entry.name()?.to_string();
            let is_dir = entry.is_dir() || raw_name.ends_with('/');
            let rel_path = sanitize_archive_rel_path(&raw_name, policy)?;
            if rel_path.is_empty() {
                continue;
            }
            let size = entry.size();
            let csize = entry.compressed_size();
            if size > policy.max_single_file_bytes {
                bail!(
                    "Zip bomb warning: entry '{}' declares {} uncompressed bytes (limit {} GiB)",
                    rel_path,
                    size,
                    policy.max_single_file_bytes / (1024 * 1024 * 1024)
                );
            }
            total_uncompressed = total_uncompressed.saturating_add(size);
            if total_uncompressed > policy.max_total_uncompressed_bytes {
                bail!(
                    "Zip bomb warning: declared total uncompressed size exceeds {} GiB",
                    policy.max_total_uncompressed_bytes / (1024 * 1024 * 1024)
                );
            }
            let encrypted = entry.encrypted();
            if encrypted {
                any_encrypted = true;
            }
            let display_name = rel_path.rsplit('/').next().unwrap_or(&rel_path).to_string();
            entries.push(ArchiveEntryInfo {
                name: display_name,
                relative_path: rel_path,
                is_dir,
                size,
                compressed_size: csize,
                modified: dos_datetime_to_system_time(entry.last_modified()),
                is_encrypted: encrypted,
            });
        }

        if total_uncompressed > policy.ratio_grace_bytes {
            let base = compressed_size.max(1);
            if (total_uncompressed as f64) > (base as f64) * policy.max_compression_ratio {
                bail!(
                    "Zip bomb warning: declared compression ratio exceeds {:.0}:1",
                    policy.max_compression_ratio
                );
            }
        }

        Ok(ArchiveInventory {
            format: ArchiveFormat::Zip,
            compressed_size,
            total_uncompressed,
            entries: Arc::new(entries),
            is_encrypted: any_encrypted,
        })
    }

    fn list_7z_entries(
        archive_path: &Path,
        compressed_size: u64,
        password: Option<&str>,
        policy: &ArchiveSafetyPolicy,
    ) -> Result<ArchiveInventory> {
        let pwd = password
            .filter(|s| !s.is_empty())
            .map(sevenz_rust::Password::from)
            .unwrap_or_else(sevenz_rust::Password::empty);
        let reader = sevenz_rust::SevenZReader::open(archive_path, pwd)
            .map_err(|e| anyhow!("7z open failed (wrong password or corrupt archive): {}", e))?;
        let files = &reader.archive().files;
        if files.len() > policy.max_entries {
            bail!(
                "7z archive contains {} entries, exceeding safety limit {}",
                files.len(),
                policy.max_entries
            );
        }
        let mut entries = Vec::with_capacity(files.len());
        let mut total_uncompressed = 0u64;
        for f in files {
            let rel_path = sanitize_archive_rel_path(f.name(), policy)?;
            if rel_path.is_empty() {
                continue;
            }
            let size = f.size();
            if size > policy.max_single_file_bytes {
                bail!("7z entry '{}' exceeds single-file size safety limit", rel_path);
            }
            total_uncompressed = total_uncompressed.saturating_add(size);
            if total_uncompressed > policy.max_total_uncompressed_bytes {
                bail!("7z archive exceeds total uncompressed size safety limit");
            }
            let display_name = rel_path.rsplit('/').next().unwrap_or(&rel_path).to_string();
            entries.push(ArchiveEntryInfo {
                name: display_name,
                relative_path: rel_path,
                is_dir: f.is_directory(),
                size,
                compressed_size: 0,
                modified: filetime_to_system_time(f.last_modified_date()),
                is_encrypted: false,
            });
        }
        Ok(ArchiveInventory {
            format: ArchiveFormat::SevenZ,
            compressed_size,
            total_uncompressed,
            entries: Arc::new(entries),
            is_encrypted: false,
        })
    }

    fn list_tar_entries(
        archive_path: &Path,
        format: ArchiveFormat,
        compressed_size: u64,
        policy: &ArchiveSafetyPolicy,
        cancel: &AtomicBool,
    ) -> Result<ArchiveInventory> {
        let total_read = AtomicU64::new(0);
        let compressed_read = Arc::new(AtomicU64::new(0));
        let mut stream = open_decompressed_stream(
            format,
            archive_path,
            compressed_read,
            &total_read,
            *policy,
            cancel,
        )?;
        let mut entries = Vec::new();
        let mut total_uncompressed = 0u64;

        visit_tar_stream(&mut stream, policy, cancel, |entry, _payload| {
            total_uncompressed = total_uncompressed.saturating_add(entry.size);
            if total_uncompressed > policy.max_total_uncompressed_bytes {
                bail!("TAR archive exceeds total uncompressed size limit");
            }
            let display_name = entry
                .rel_path
                .rsplit('/')
                .next()
                .unwrap_or(&entry.rel_path)
                .to_string();
            entries.push(ArchiveEntryInfo {
                name: display_name,
                relative_path: entry.rel_path,
                is_dir: entry.is_dir,
                size: entry.size,
                compressed_size: 0,
                modified: entry.mtime,
                is_encrypted: false,
            });
            Ok(true)
        })?;

        Ok(ArchiveInventory {
            format,
            compressed_size,
            total_uncompressed,
            entries: Arc::new(entries),
            is_encrypted: false,
        })
    }

    /// Returns Windows Explorer-style virtual `FileItem` children for `inner_dir` inside an archive.
    pub fn list_virtual_directory(archive_path: &Path, inner_dir: &str) -> Result<Vec<FileItem>> {
        let inventory = Self::list_entries_detailed(archive_path, None)?;
        let prefix = normalize_virtual_dir(inner_dir);
        let prefix_slash = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix)
        };

        let mut children: BTreeMap<String, ArchiveEntryInfo> = BTreeMap::new();
        for entry in inventory.entries.iter() {
            let rel = entry.relative_path.as_str();
            let sub = if prefix_slash.is_empty() {
                rel
            } else if let Some(rest) = rel.strip_prefix(&prefix_slash) {
                rest
            } else {
                continue;
            };
            if sub.is_empty() {
                continue;
            }
            if let Some((child_dir, _rest)) = sub.split_once('/') {
                let child_rel = if prefix.is_empty() {
                    child_dir.to_string()
                } else {
                    format!("{}/{}", prefix, child_dir)
                };
                children.entry(child_dir.to_string()).or_insert_with(|| ArchiveEntryInfo {
                    name: child_dir.to_string(),
                    relative_path: child_rel,
                    is_dir: true,
                    size: 0,
                    compressed_size: 0,
                    modified: entry.modified,
                    is_encrypted: entry.is_encrypted,
                });
            } else {
                let child_rel = if prefix.is_empty() {
                    sub.to_string()
                } else {
                    format!("{}/{}", prefix, sub)
                };
                let existing_is_dir = entry.is_dir;
                children
                    .entry(sub.to_string())
                    .and_modify(|e| {
                        if !existing_is_dir {
                            e.size = entry.size;
                            e.compressed_size = entry.compressed_size;
                            e.is_dir = false;
                            e.modified = entry.modified;
                        }
                    })
                    .or_insert_with(|| ArchiveEntryInfo {
                        name: sub.to_string(),
                        relative_path: child_rel,
                        is_dir: entry.is_dir,
                        size: entry.size,
                        compressed_size: entry.compressed_size,
                        modified: entry.modified,
                        is_encrypted: entry.is_encrypted,
                    });
            }
        }

        let mut items: Vec<FileItem> = children
            .into_values()
            .map(|entry| {
                let mut virtual_path = archive_path.to_path_buf();
                for seg in entry.relative_path.split('/').filter(|s| !s.is_empty()) {
                    virtual_path.push(seg);
                }
                let ftype = if entry.is_dir {
                    FileType::Directory
                } else {
                    FileType::File
                };
                let is_hidden = entry.name.starts_with('.');
                FileItem {
                    name: entry.name,
                    path: FilesPath::new(virtual_path),
                    file_type: ftype,
                    size: entry.size,
                    is_hidden,
                    is_system: false,
                    modified: entry.modified,
                    created: entry.modified,
                    tags: Vec::new(),
                }
            })
            .collect();

        items.sort_by(|a, b| {
            matches!(b.file_type, FileType::Directory)
                .cmp(&matches!(a.file_type, FileType::Directory))
                .then_with(|| crate::ui::views::helpers::natural_cmp(&a.name, &b.name))
        });
        Ok(items)
    }

    /// Safely extracts a single file entry from an archive into a temporary view folder
    /// (capped at `max_bytes` to prevent opening a zip bomb in-place).
    pub fn extract_single_entry_to_temp(
        archive_path: &Path,
        rel_path: &str,
        max_bytes: u64,
        password: Option<&str>,
    ) -> Result<PathBuf> {
        let mut policy = ArchiveSafetyPolicy::default();
        policy.max_single_file_bytes = max_bytes;
        policy.max_total_uncompressed_bytes = max_bytes;
        let clean_target = sanitize_archive_rel_path(rel_path, &policy)?;
        if clean_target.is_empty() {
            bail!("Invalid empty archive entry path");
        }

        let format = ArchiveFormat::detect(archive_path)
            .ok_or_else(|| anyhow!("Unsupported archive format"))?;
        let meta = std::fs::metadata(archive_path)?;
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        archive_path.hash(&mut hasher);
        clean_target.hash(&mut hasher);
        meta.len().hash(&mut hasher);
        mtime_secs.hash(&mut hasher);
        let key_hex = format!("{:016x}", hasher.finish());

        let file_name = clean_target.rsplit('/').next().unwrap_or("entry.bin");
        let temp_dir = std::env::temp_dir()
            .join("files-rs-archive-view")
            .join(key_hex);
        std::fs::create_dir_all(&temp_dir)?;
        let out_path = temp_dir.join(file_name);
        if out_path.is_file() {
            return Ok(out_path);
        }

        let cancel = AtomicBool::new(false);
        let total_read = AtomicU64::new(0);
        let compressed_read = Arc::new(AtomicU64::new(meta.len().min(1024)));

        match format {
            ArchiveFormat::Zip => {
                let file = File::open(archive_path)?;
                let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
                for i in 0..archive.len() {
                    let entry_name = {
                        let raw = archive.by_index_raw(i)?;
                        sanitize_archive_rel_path(&raw.name()?, &policy).unwrap_or_default()
                    };
                    if entry_name == clean_target {
                        let mut zf = if let Some(pwd) = password.filter(|s| !s.is_empty()) {
                            match archive.by_index_decrypt(i, pwd.as_bytes()) {
                                Ok(f) => f,
                                Err(_) => bail!("Incorrect ZIP password or unsupported cipher"),
                            }
                        } else {
                            match archive.by_index(i) {
                                Ok(f) => f,
                                Err(zip::result::ZipError::UnsupportedArchive(msg))
                                    if msg.to_ascii_lowercase().contains("password") =>
                                {
                                    bail!("Archive entry '{}' is password-protected", clean_target)
                                }
                                Err(e) => return Err(e.into()),
                            }
                        };
                        let csize = zf.compressed_size().max(1);
                        let compressed_atomic = Arc::new(AtomicU64::new(csize));
                        let mut out = File::create(&out_path)?;
                        copy_bounded_entry(
                            &mut zf,
                            &mut out,
                            max_bytes,
                            &total_read,
                            &compressed_atomic,
                            &policy,
                            &cancel,
                        )?;
                        return Ok(out_path);
                    }
                }
                bail!("Entry '{}' not found in ZIP archive", clean_target);
            }
            ArchiveFormat::SevenZ => {
                let pwd = password
                    .filter(|s| !s.is_empty())
                    .map(sevenz_rust::Password::from)
                    .unwrap_or_else(sevenz_rust::Password::empty);
                let mut reader = sevenz_rust::SevenZReader::open(archive_path, pwd)
                    .map_err(|e| anyhow!("7z error: {}", e))?;
                let mut found = false;
                let mut copy_err: Option<anyhow::Error> = None;
                reader
                    .for_each_entries(|entry, r| {
                        let rel = sanitize_archive_rel_path(entry.name(), &policy)
                            .unwrap_or_default();
                        if rel == clean_target && !entry.is_directory() {
                            found = true;
                            let mut out = match File::create(&out_path) {
                                Ok(f) => f,
                                Err(e) => {
                                    copy_err = Some(e.into());
                                    return Ok(false);
                                }
                            };
                            if let Err(e) = copy_bounded_entry(
                                &mut { r },
                                &mut out,
                                max_bytes,
                                &total_read,
                                &compressed_read,
                                &policy,
                                &cancel,
                            ) {
                                copy_err = Some(e);
                                return Ok(false);
                            }
                            return Ok(false);
                        }
                        Ok(true)
                    })
                    .map_err(|e| anyhow!("7z extract error: {}", e))?;
                if let Some(err) = copy_err {
                    let _ = std::fs::remove_file(&out_path);
                    return Err(err);
                }
                if !found {
                    bail!("Entry '{}' not found in 7z archive", clean_target);
                }
                Ok(out_path)
            }
            ArchiveFormat::Tar
            | ArchiveFormat::TarGz
            | ArchiveFormat::TarBz2
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarZst
            | ArchiveFormat::TarLzma => {
                let c_atomic = Arc::new(AtomicU64::new(0));
                let mut stream = open_decompressed_stream(
                    format,
                    archive_path,
                    c_atomic.clone(),
                    &total_read,
                    ArchiveSafetyPolicy {
                        max_single_file_bytes: max_bytes,
                        max_total_uncompressed_bytes: policy.max_total_uncompressed_bytes.max(512 * 1024 * 1024),
                        ..ArchiveSafetyPolicy::default()
                    },
                    &cancel,
                )?;
                let mut found = false;
                visit_tar_stream(&mut stream, &policy, &cancel, |entry, payload| {
                    if entry.rel_path == clean_target && entry.is_regular_file {
                        found = true;
                        let mut out = File::create(&out_path)?;
                        let n = std::io::copy(&mut payload.take(max_bytes + 1), &mut out)?;
                        if n > max_bytes {
                            let _ = std::fs::remove_file(&out_path);
                            bail!("Entry exceeds {} MB safety preview limit", max_bytes / (1024 * 1024));
                        }
                        return Ok(false);
                    }
                    Ok(true)
                })?;
                if !found {
                    bail!("Entry '{}' not found in archive", clean_target);
                }
                Ok(out_path)
            }
            ArchiveFormat::Gzip
            | ArchiveFormat::Bzip2
            | ArchiveFormat::Xz
            | ArchiveFormat::Zstd => {
                let c_atomic = Arc::new(AtomicU64::new(0));
                let stream = open_decompressed_stream(
                    format,
                    archive_path,
                    c_atomic,
                    &total_read,
                    policy,
                    &cancel,
                )?;
                let mut out = File::create(&out_path)?;
                let n = std::io::copy(&mut stream.take(max_bytes + 1), &mut out)?;
                if n > max_bytes {
                    let _ = std::fs::remove_file(&out_path);
                    bail!("File exceeds {} MB preview/open safety limit", max_bytes / (1024 * 1024));
                }
                Ok(out_path)
            }
        }
    }

    /// Extracts any supported archive format using `ExtractOptions`, zip-bomb protection,
    /// path-traversal checks, cancellation, and progress callbacks.
    pub fn extract_archive_with_options<F>(
        opts: &ExtractOptions,
        mut on_progress: F,
        cancel: &AtomicBool,
    ) -> Result<ExtractReport>
    where
        F: FnMut(ExtractProgress),
    {
        let format = ArchiveFormat::detect(&opts.archive_path).ok_or_else(|| {
            anyhow!(
                "Unsupported archive format: {}",
                opts.archive_path.display()
            )
        })?;

        let final_dest = if opts.create_subfolder {
            let sub = ArchiveFormat::suggested_folder_name(&opts.archive_path);
            let safe_sub = sanitize_archive_rel_path(&sub, &opts.policy)
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "extracted".to_string());
            opts.destination.join(safe_sub)
        } else {
            opts.destination.clone()
        };
        std::fs::create_dir_all(&final_dest)?;

        let total_read = AtomicU64::new(0);
        let meta_size = std::fs::metadata(&opts.archive_path)
            .map(|m| m.len())
            .unwrap_or(1)
            .max(1);
        let compressed_atomic = Arc::new(AtomicU64::new(meta_size));

        match format {
            ArchiveFormat::Zip => Self::extract_zip_with_opts(
                opts,
                &final_dest,
                &total_read,
                &mut on_progress,
                cancel,
            ),
            ArchiveFormat::SevenZ => Self::extract_7z_with_opts(
                opts,
                &final_dest,
                &total_read,
                &compressed_atomic,
                &mut on_progress,
                cancel,
            ),
            ArchiveFormat::Tar
            | ArchiveFormat::TarGz
            | ArchiveFormat::TarBz2
            | ArchiveFormat::TarXz
            | ArchiveFormat::TarZst
            | ArchiveFormat::TarLzma => Self::extract_tar_with_opts(
                opts,
                format,
                &final_dest,
                &total_read,
                &mut on_progress,
                cancel,
            ),
            ArchiveFormat::Gzip
            | ArchiveFormat::Bzip2
            | ArchiveFormat::Xz
            | ArchiveFormat::Zstd => {
                let inner_name = single_stream_inner_name(&opts.archive_path, format);
                if let Some(ref sel) = opts.selected_entries {
                    if !sel.is_empty() && !sel.contains(&inner_name) {
                        return Ok(ExtractReport {
                            final_destination: final_dest,
                            extracted_files: 0,
                            extracted_bytes: 0,
                        });
                    }
                }
                let out_path = final_dest.join(&inner_name);
                ensure_within_root(&final_dest, &out_path)?;
                if out_path.exists() && !opts.overwrite {
                    return Ok(ExtractReport {
                        final_destination: final_dest,
                        extracted_files: 0,
                        extracted_bytes: 0,
                    });
                }
                let c_atomic = Arc::new(AtomicU64::new(0));
                let mut stream = open_decompressed_stream(
                    format,
                    &opts.archive_path,
                    c_atomic,
                    &total_read,
                    opts.policy,
                    cancel,
                )?;
                let mut out = File::create(&out_path)?;
                let written = std::io::copy(&mut stream, &mut out)?;
                on_progress(ExtractProgress {
                    current_item: inner_name,
                    entries_done: 1,
                    entries_total: 1,
                    bytes_done: written,
                    bytes_total: written,
                });
                Ok(ExtractReport {
                    final_destination: final_dest,
                    extracted_files: 1,
                    extracted_bytes: written,
                })
            }
        }
    }

    fn matches_selection(selected: Option<&HashSet<String>>, rel_path: &str) -> bool {
        let Some(sel) = selected else {
            return true;
        };
        if sel.is_empty() || sel.contains(rel_path) {
            return true;
        }
        // Also match if user selected a parent directory in the checklist
        for s in sel {
            let trimmed = s.trim_end_matches('/');
            if rel_path == trimmed || rel_path.starts_with(&format!("{}/", trimmed)) {
                return true;
            }
        }
        false
    }

    fn extract_zip_with_opts<F>(
        opts: &ExtractOptions,
        final_dest: &Path,
        total_read: &AtomicU64,
        on_progress: &mut F,
        cancel: &AtomicBool,
    ) -> Result<ExtractReport>
    where
        F: FnMut(ExtractProgress),
    {
        let file = File::open(&opts.archive_path)?;
        let mut archive = zip::ZipArchive::new(BufReader::new(file))?;
        let total_entries = archive.len();
        if total_entries > opts.policy.max_entries {
            bail!("Archive exceeds maximum entry count ({})", opts.policy.max_entries);
        }

        let mut total_declared_bytes = 0u64;
        for i in 0..total_entries {
            if let Ok(raw) = archive.by_index_raw(i) {
                total_declared_bytes = total_declared_bytes.saturating_add(raw.size());
            }
        }

        let mut extracted_files = 0usize;
        let mut extracted_bytes = 0u64;

        for i in 0..total_entries {
            if cancel.load(Ordering::Relaxed) {
                bail!("Extraction cancelled");
            }
            let (raw_name, is_dir, csize) = {
                let raw = archive.by_index_raw(i)?;
                (raw.name()?.to_string(), raw.is_dir(), raw.compressed_size())
            };
            let rel_path = sanitize_archive_rel_path(&raw_name, &opts.policy)?;
            if rel_path.is_empty()
                || !Self::matches_selection(opts.selected_entries.as_ref(), &rel_path)
            {
                continue;
            }

            let outpath = final_dest.join(&rel_path);
            ensure_within_root(final_dest, &outpath)?;

            if is_dir || raw_name.ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if outpath.exists() && !opts.overwrite {
                    continue;
                }
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut zf = if let Some(ref pwd) = opts.password.as_ref().filter(|s| !s.is_empty())
                {
                    archive
                        .by_index_decrypt(i, pwd.as_bytes())
                        .map_err(|_| anyhow!("Incorrect ZIP password or corrupt entry"))?
                } else {
                    match archive.by_index(i) {
                        Ok(f) => f,
                        Err(zip::result::ZipError::UnsupportedArchive(msg))
                            if msg.to_ascii_lowercase().contains("password") =>
                        {
                            bail!("Archive '{}' is encrypted — please enter the password", rel_path)
                        }
                        Err(e) => return Err(e.into()),
                    }
                };

                let comp_atomic = Arc::new(AtomicU64::new(csize.max(1)));
                let mut outfile = File::create(&outpath)?;
                let written = match copy_bounded_entry(
                    &mut zf,
                    &mut outfile,
                    opts.policy.max_single_file_bytes,
                    total_read,
                    &comp_atomic,
                    &opts.policy,
                    cancel,
                ) {
                    Ok(n) => n,
                    Err(e) => {
                        let _ = std::fs::remove_file(&outpath);
                        return Err(e);
                    }
                };
                extracted_files += 1;
                extracted_bytes = extracted_bytes.saturating_add(written);
            }

            if i % 8 == 0 || i + 1 == total_entries {
                on_progress(ExtractProgress {
                    current_item: rel_path,
                    entries_done: i + 1,
                    entries_total: total_entries,
                    bytes_done: extracted_bytes,
                    bytes_total: total_declared_bytes.max(extracted_bytes),
                });
            }
        }

        Ok(ExtractReport {
            final_destination: final_dest.to_path_buf(),
            extracted_files,
            extracted_bytes,
        })
    }

    fn extract_7z_with_opts<F>(
        opts: &ExtractOptions,
        final_dest: &Path,
        total_read: &AtomicU64,
        compressed_atomic: &Arc<AtomicU64>,
        on_progress: &mut F,
        cancel: &AtomicBool,
    ) -> Result<ExtractReport>
    where
        F: FnMut(ExtractProgress),
    {
        let pwd = opts
            .password
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(sevenz_rust::Password::from)
            .unwrap_or_else(sevenz_rust::Password::empty);
        let mut reader = sevenz_rust::SevenZReader::open(&opts.archive_path, pwd)
            .map_err(|e| anyhow!("7z error: {}", e))?;
        let entries_total = reader.archive().files.len();
        if entries_total > opts.policy.max_entries {
            bail!("7z archive exceeds max entries safety limit");
        }
        let bytes_total: u64 = reader.archive().files.iter().map(|f| f.size()).sum();

        let mut entries_done = 0usize;
        let mut extracted_files = 0usize;
        let mut extracted_bytes = 0u64;
        let mut err_holder: Option<anyhow::Error> = None;

        reader
            .for_each_entries(|entry, r| {
                if cancel.load(Ordering::Relaxed) {
                    err_holder = Some(anyhow!("Extraction cancelled"));
                    return Ok(false);
                }
                entries_done += 1;
                let rel = match sanitize_archive_rel_path(entry.name(), &opts.policy) {
                    Ok(p) => p,
                    Err(e) => {
                        err_holder = Some(e);
                        return Ok(false);
                    }
                };
                if rel.is_empty()
                    || !Self::matches_selection(opts.selected_entries.as_ref(), &rel)
                {
                    return Ok(true);
                }
                let outpath = final_dest.join(&rel);
                if let Err(e) = ensure_within_root(final_dest, &outpath) {
                    err_holder = Some(e);
                    return Ok(false);
                }
                if entry.is_directory() {
                    if let Err(e) = std::fs::create_dir_all(&outpath) {
                        err_holder = Some(e.into());
                        return Ok(false);
                    }
                } else {
                    if outpath.exists() && !opts.overwrite {
                        return Ok(true);
                    }
                    if let Some(p) = outpath.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let mut outfile = match File::create(&outpath) {
                        Ok(f) => f,
                        Err(e) => {
                            err_holder = Some(e.into());
                            return Ok(false);
                        }
                    };
                    match copy_bounded_entry(
                        &mut { r },
                        &mut outfile,
                        opts.policy.max_single_file_bytes,
                        total_read,
                        compressed_atomic,
                        &opts.policy,
                        cancel,
                    ) {
                        Ok(written) => {
                            extracted_files += 1;
                            extracted_bytes = extracted_bytes.saturating_add(written);
                        }
                        Err(e) => {
                            let _ = std::fs::remove_file(&outpath);
                            err_holder = Some(e);
                            return Ok(false);
                        }
                    }
                }
                if entries_done % 8 == 0 || entries_done == entries_total {
                    on_progress(ExtractProgress {
                        current_item: rel,
                        entries_done,
                        entries_total,
                        bytes_done: extracted_bytes,
                        bytes_total: bytes_total.max(extracted_bytes),
                    });
                }
                Ok(true)
            })
            .map_err(|e| anyhow!("7z extraction failed: {}", e))?;

        if let Some(e) = err_holder {
            return Err(e);
        }

        Ok(ExtractReport {
            final_destination: final_dest.to_path_buf(),
            extracted_files,
            extracted_bytes,
        })
    }

    fn extract_tar_with_opts<F>(
        opts: &ExtractOptions,
        format: ArchiveFormat,
        final_dest: &Path,
        total_read: &AtomicU64,
        on_progress: &mut F,
        cancel: &AtomicBool,
    ) -> Result<ExtractReport>
    where
        F: FnMut(ExtractProgress),
    {
        let comp_atomic = Arc::new(AtomicU64::new(0));
        let mut stream = open_decompressed_stream(
            format,
            &opts.archive_path,
            comp_atomic,
            total_read,
            opts.policy,
            cancel,
        )?;
        let mut entries_done = 0usize;
        let mut extracted_files = 0usize;
        let mut extracted_bytes = 0u64;

        visit_tar_stream(&mut stream, &opts.policy, cancel, |entry, payload| {
            entries_done += 1;
            if !Self::matches_selection(opts.selected_entries.as_ref(), &entry.rel_path) {
                return Ok(true);
            }
            let outpath = final_dest.join(&entry.rel_path);
            ensure_within_root(final_dest, &outpath)?;

            if entry.is_dir {
                std::fs::create_dir_all(&outpath)?;
            } else if entry.is_regular_file {
                if outpath.exists() && !opts.overwrite {
                    return Ok(true);
                }
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                let mut exact = payload.take(entry.size);
                let n = std::io::copy(&mut exact, &mut outfile)?;
                extracted_files += 1;
                extracted_bytes = extracted_bytes.saturating_add(n);
            }

            if entries_done % 16 == 0 {
                on_progress(ExtractProgress {
                    current_item: entry.rel_path,
                    entries_done,
                    entries_total: entries_done,
                    bytes_done: extracted_bytes,
                    bytes_total: extracted_bytes,
                });
            }
            Ok(true)
        })?;

        on_progress(ExtractProgress {
            current_item: String::new(),
            entries_done,
            entries_total: entries_done,
            bytes_done: extracted_bytes,
            bytes_total: extracted_bytes,
        });

        Ok(ExtractReport {
            final_destination: final_dest.to_path_buf(),
            extracted_files,
            extracted_bytes,
        })
    }

    /// Creates a ZIP archive from a list of files/folders
    pub async fn create_zip(source_items: &[FilesPath], destination: &FilesPath) -> Result<()> {
        let path = std::path::PathBuf::from(destination.as_path());
        let file = File::create(&path)?;
        let mut zip = ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for item in source_items {
            let item_path = item.as_path();
            if item_path.is_dir() {
                let walk = jwalk::WalkDir::new(item_path)
                    .into_iter()
                    .filter_map(|e| e.ok());
                for entry in walk {
                    let path = entry.path();
                    let name = path
                        .strip_prefix(item_path)
                        .map_err(|e| anyhow!("Failed to strip prefix: {}", e))?;

                    if path.is_dir() {
                        zip.add_directory(name.to_string_lossy(), options.clone())?;
                    } else {
                        zip.start_file(name.to_string_lossy(), options.clone())?;
                        let mut f = std::fs::File::open(path)?;
                        std::io::copy(&mut f, &mut zip)?;
                    }
                }
            } else {
                zip.start_file(
                    item_path.file_name().unwrap().to_string_lossy(),
                    options.clone(),
                )?;
                let mut f = std::fs::File::open(item_path)?;
                std::io::copy(&mut f, &mut zip)?;
            }
        }
        zip.finish()?;
        Ok(())
    }

    /// Extracts an archive to a destination folder with zip-bomb & traversal protection
    pub async fn extract_zip(archive_path: &FilesPath, destination: &FilesPath) -> Result<()> {
        let opts = ExtractOptions {
            archive_path: archive_path.as_path().to_path_buf(),
            destination: destination.as_path().to_path_buf(),
            selected_entries: None,
            create_subfolder: false,
            overwrite: true,
            password: None,
            policy: ArchiveSafetyPolicy::default(),
        };
        let cancel = AtomicBool::new(false);
        Self::extract_archive_with_options(&opts, |_| {}, &cancel)?;
        Ok(())
    }

    /// Lists contents of any supported archive without extracting payloads
    pub fn list_contents(archive_path: &FilesPath) -> Result<Vec<String>> {
        let inv = Self::list_entries_detailed(archive_path.as_path(), None)?;
        Ok(inv
            .entries
            .iter()
            .map(|e| e.relative_path.clone())
            .collect())
    }

    /// Extracts archive "smart" - creates subfolder if multiple top-level items exist
    pub async fn extract_zip_smart(
        archive_path: &FilesPath,
        destination: &FilesPath,
    ) -> Result<FilesPath> {
        let analysis = Self::analyze_archive(archive_path)?;
        let actual_destination = if analysis.top_level_count > 1 {
            let archive_name = ArchiveFormat::suggested_folder_name(archive_path.as_path());
            let subfolder_path = destination.as_path().join(&archive_name);
            std::fs::create_dir_all(&subfolder_path)?;
            FilesPath::new(subfolder_path)
        } else {
            destination.clone()
        };

        Self::extract_zip(archive_path, &actual_destination).await?;
        Ok(actual_destination)
    }

    /// Analyzes any supported archive to determine extraction strategy
    pub fn analyze_archive(archive_path: &FilesPath) -> Result<ArchiveAnalysis> {
        let inv = Self::list_entries_detailed(archive_path.as_path(), None)?;
        let total_items = inv.entries.len();

        let mut top_level = Vec::new();
        let mut seen = HashSet::new();
        for e in inv.entries.iter() {
            let top = e
                .relative_path
                .split('/')
                .next()
                .unwrap_or(&e.relative_path)
                .to_string();
            if !top.is_empty() && seen.insert(top.clone()) {
                top_level.push(top);
            }
        }

        let strategy = if top_level.is_empty() {
            ExtractionStrategy::Empty
        } else if top_level.len() == 1 {
            let first = &top_level[0];
            let has_only_dir = inv
                .entries
                .iter()
                .any(|e| e.relative_path == *first && e.is_dir)
                || inv
                    .entries
                    .iter()
                    .any(|e| e.relative_path.starts_with(&format!("{}/", first)));
            if has_only_dir {
                ExtractionStrategy::SingleFolder
            } else {
                ExtractionStrategy::SingleFile
            }
        } else {
            ExtractionStrategy::MultipleItems(top_level.len())
        };

        Ok(ArchiveAnalysis {
            total_items,
            top_level_count: top_level.len(),
            top_level_names: top_level,
            recommended_strategy: strategy,
        })
    }
}


/// Analysis result for an archive
#[derive(Debug, Clone)]
pub struct ArchiveAnalysis {
    pub total_items: usize,
    pub top_level_count: usize,
    pub top_level_names: Vec<String>,
    pub recommended_strategy: ExtractionStrategy,
}

/// Recommended extraction strategy based on archive contents
#[derive(Debug, Clone, PartialEq)]
pub enum ExtractionStrategy {
    /// Archive is empty
    Empty,
    /// Single file directly in archive
    SingleFile,
    /// Single folder directly in archive
    SingleFolder,
    /// Multiple items at root level
    MultipleItems(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_and_extract_zip_file() -> Result<()> {
        let dir = tempdir()?;

        // Create test files
        let file1 = dir.path().join("test1.txt");
        let file2 = dir.path().join("test2.txt");
        std::fs::write(&file1, "Hello World 1")?;
        std::fs::write(&file2, "Hello World 2")?;

        // Create zip
        let zip_path = dir.path().join("output.zip");
        let source_items = vec![FilesPath::new(&file1), FilesPath::new(&file2)];

        ArchiveService::create_zip(&source_items, &FilesPath::new(&zip_path)).await?;

        // Verify zip exists
        assert!(zip_path.exists());

        // Extract to new folder
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Verify extracted files
        assert!(extract_dir.join("test1.txt").exists());
        assert!(extract_dir.join("test2.txt").exists());
        assert_eq!(
            std::fs::read_to_string(extract_dir.join("test1.txt"))?,
            "Hello World 1"
        );
        assert_eq!(
            std::fs::read_to_string(extract_dir.join("test2.txt"))?,
            "Hello World 2"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_create_zip_directory() -> Result<()> {
        let dir = tempdir()?;

        // Create a folder structure
        let subfolder = dir.path().join("docs");
        std::fs::create_dir(&subfolder)?;
        std::fs::write(subfolder.join("readme.txt"), "Readme content")?;
        std::fs::write(subfolder.join("notes.txt"), "Notes content")?;

        // Create zip from directory - contents are stored without the folder prefix
        let zip_path = dir.path().join("docs.zip");
        let source_items = vec![FilesPath::new(&subfolder)];

        ArchiveService::create_zip(&source_items, &FilesPath::new(&zip_path)).await?;

        assert!(zip_path.exists());

        // Extract and verify
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Files are stored without folder prefix, so they go directly to extract_dir
        assert!(extract_dir.join("readme.txt").exists());
        assert!(extract_dir.join("notes.txt").exists());

        Ok(())
    }

    #[test]
    fn test_list_contents() -> Result<()> {
        let dir = tempdir()?;

        // Create test files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        std::fs::write(&file1, "Content 1")?;
        std::fs::write(&file2, "Content 2")?;

        // Create zip
        let zip_path = dir.path().join("output.zip");
        let source_items = vec![FilesPath::new(&file1), FilesPath::new(&file2)];

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(
            &source_items,
            &FilesPath::new(&zip_path),
        ))?;

        // List contents
        let contents = ArchiveService::list_contents(&FilesPath::new(&zip_path))?;

        assert_eq!(contents.len(), 2);
        assert!(contents.contains(&"file1.txt".to_string()));
        assert!(contents.contains(&"file2.txt".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_archive() -> Result<()> {
        let dir = tempdir()?;

        // Create empty zip
        let zip_path = dir.path().join("empty.zip");
        ArchiveService::create_zip(&[], &FilesPath::new(&zip_path)).await?;

        assert!(zip_path.exists());

        // Extract empty zip
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Should have extracted nothing
        let entries: Vec<_> = std::fs::read_dir(&extract_dir)?.collect();
        assert!(entries.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_preserve_directory_structure() -> Result<()> {
        let dir = tempdir()?;

        // Create nested folder structure
        let folder1 = dir.path().join("folder1");
        let folder2 = folder1.join("folder2");
        std::fs::create_dir_all(&folder2)?;

        std::fs::write(folder2.join("deep_file.txt"), "Deep content")?;
        std::fs::write(folder1.join("shallow_file.txt"), "Shallow content")?;

        // Create zip from nested folder
        let zip_path = dir.path().join("nested.zip");
        let source_items = vec![FilesPath::new(&folder1)];

        ArchiveService::create_zip(&source_items, &FilesPath::new(&zip_path)).await?;

        // Extract
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        ArchiveService::extract_zip(&FilesPath::new(&zip_path), &FilesPath::new(&extract_dir))
            .await?;

        // Verify directory structure - prefix is stripped so files go to root
        // folder2/deep_file.txt becomes folder2/deep_file.txt (prefix folder1 stripped)
        // folder1/shallow_file.txt becomes shallow_file.txt (prefix folder1 stripped)
        assert!(extract_dir.join("shallow_file.txt").exists());
        assert!(extract_dir.join("folder2").join("deep_file.txt").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_extract_zip_smart_single_file() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with a single file
        let file1 = dir.path().join("readme.txt");
        std::fs::write(&file1, "Readme content")?;

        let zip_path = dir.path().join("single.zip");
        ArchiveService::create_zip(&[FilesPath::new(&file1)], &FilesPath::new(&zip_path)).await?;

        // Extract with smart method
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        let result_path = ArchiveService::extract_zip_smart(
            &FilesPath::new(&zip_path),
            &FilesPath::new(&extract_dir),
        )
        .await?;

        // Should extract directly to extract_dir (no subfolder for single file)
        assert!(extract_dir.join("readme.txt").exists());
        assert_eq!(result_path.as_path(), extract_dir);

        Ok(())
    }

    #[tokio::test]
    async fn test_extract_zip_smart_multiple_files() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with multiple files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        std::fs::write(&file1, "Content 1")?;
        std::fs::write(&file2, "Content 2")?;

        let zip_path = dir.path().join("multi.zip");
        ArchiveService::create_zip(
            &[FilesPath::new(&file1), FilesPath::new(&file2)],
            &FilesPath::new(&zip_path),
        )
        .await?;

        // Extract with smart method
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir(&extract_dir)?;

        let result_path = ArchiveService::extract_zip_smart(
            &FilesPath::new(&zip_path),
            &FilesPath::new(&extract_dir),
        )
        .await?;

        // Should create subfolder "multi" (from archive name)
        assert!(result_path.as_path().join("file1.txt").exists());
        assert!(result_path.as_path().join("file2.txt").exists());
        assert!(result_path.as_path().ends_with("multi"));

        Ok(())
    }

    #[test]
    fn test_analyze_archive_single_file() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with a single file
        let file1 = dir.path().join("readme.txt");
        std::fs::write(&file1, "Readme")?;

        let zip_path = dir.path().join("single.zip");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(
            &[FilesPath::new(&file1)],
            &FilesPath::new(&zip_path),
        ))?;

        let analysis = ArchiveService::analyze_archive(&FilesPath::new(&zip_path))?;

        assert_eq!(analysis.total_items, 1);
        assert_eq!(analysis.top_level_count, 1);
        assert!(matches!(
            analysis.recommended_strategy,
            ExtractionStrategy::SingleFile
        ));

        Ok(())
    }

    #[test]
    fn test_analyze_archive_multiple_items() -> Result<()> {
        let dir = tempdir()?;

        // Create a zip with multiple files
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        std::fs::write(&file1, "1")?;
        std::fs::write(&file2, "2")?;

        let zip_path = dir.path().join("multi.zip");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(
            &[FilesPath::new(&file1), FilesPath::new(&file2)],
            &FilesPath::new(&zip_path),
        ))?;

        let analysis = ArchiveService::analyze_archive(&FilesPath::new(&zip_path))?;

        assert_eq!(analysis.total_items, 2);
        assert_eq!(analysis.top_level_count, 2);
        assert!(matches!(
            analysis.recommended_strategy,
            ExtractionStrategy::MultipleItems(2)
        ));

        Ok(())
    }

    #[test]
    fn test_analyze_archive_empty() -> Result<()> {
        let dir = tempdir()?;

        // Create empty zip
        let zip_path = dir.path().join("empty.zip");
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(ArchiveService::create_zip(&[], &FilesPath::new(&zip_path)))?;

        let analysis = ArchiveService::analyze_archive(&FilesPath::new(&zip_path))?;

        assert_eq!(analysis.total_items, 0);
        assert_eq!(analysis.top_level_count, 0);
        assert!(matches!(
            analysis.recommended_strategy,
            ExtractionStrategy::Empty
        ));

        Ok(())
    }

    fn write_ustar_header(name: &str, size: u64, typeflag: u8) -> [u8; 512] {
        let mut blk = [0u8; 512];
        let nb = name.as_bytes();
        let copy_len = nb.len().min(100);
        blk[..copy_len].copy_from_slice(&nb[..copy_len]);
        blk[100..108].copy_from_slice(b"0000644\0");
        blk[108..116].copy_from_slice(b"0000000\0");
        blk[116..124].copy_from_slice(b"0000000\0");
        let sz = format!("{:011o}\0", size);
        blk[124..136].copy_from_slice(sz.as_bytes());
        let mt = format!("{:011o}\0", 1_700_000_000u64);
        blk[136..148].copy_from_slice(mt.as_bytes());
        blk[148..156].copy_from_slice(b"        ");
        blk[156] = typeflag;
        blk[257..263].copy_from_slice(b"ustar\0");
        blk[263..265].copy_from_slice(b"00");
        let cksum: u32 = blk.iter().map(|&b| u32::from(b)).sum();
        let ck_str = format!("{:06o}\0 ", cksum);
        blk[148..156].copy_from_slice(ck_str.as_bytes());
        blk
    }

    fn build_tar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar = Vec::new();
        for (name, data) in entries {
            let hdr = write_ustar_header(name, data.len() as u64, b'0');
            tar.extend_from_slice(&hdr);
            tar.extend_from_slice(data);
            let rem = data.len() % 512;
            if rem != 0 {
                tar.resize(tar.len() + (512 - rem), 0);
            }
        }
        tar.resize(tar.len() + 1024, 0);
        tar
    }

    #[test]
    fn test_tar_gz_virtual_browse_and_extract() -> Result<()> {
        let dir = tempdir()?;
        let tar_gz_path = dir.path().join("project.tar.gz");
        let tar_bytes = build_tar_bytes(&[
            ("src/main.rs", b"fn main() {}"),
            ("README.md", b"# Hello"),
        ]);
        {
            let f = File::create(&tar_gz_path)?;
            let mut gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            gz.write_all(&tar_bytes)?;
            gz.finish()?;
        }

        assert_eq!(
            ArchiveService::detect_format(&tar_gz_path),
            Some(ArchiveFormat::TarGz)
        );
        let root_items = ArchiveService::list_virtual_directory(&tar_gz_path, "")?;
        assert_eq!(root_items.len(), 2);
        assert!(root_items
            .iter()
            .any(|i| i.name == "src" && matches!(i.file_type, FileType::Directory)));
        assert!(root_items
            .iter()
            .any(|i| i.name == "README.md" && matches!(i.file_type, FileType::File)));

        let nested_path = tar_gz_path.join("src");
        let (arch, inner) = ArchiveService::split_archive_path(&nested_path).unwrap();
        assert_eq!(arch, tar_gz_path);
        assert_eq!(inner, "src");
        let src_items = ArchiveService::list_virtual_directory(&arch, &inner)?;
        assert_eq!(src_items.len(), 1);
        assert_eq!(src_items[0].name, "main.rs");

        let temp_file = ArchiveService::extract_single_entry_to_temp(
            &tar_gz_path,
            "src/main.rs",
            1024 * 1024,
            None,
        )?;
        assert_eq!(std::fs::read_to_string(temp_file)?, "fn main() {}");

        let dest = dir.path().join("out");
        let report = ArchiveService::extract_archive_with_options(
            &ExtractOptions {
                archive_path: tar_gz_path,
                destination: dest.clone(),
                selected_entries: None,
                create_subfolder: true,
                overwrite: true,
                password: None,
                policy: ArchiveSafetyPolicy::default(),
            },
            |_| {},
            &AtomicBool::new(false),
        )?;
        assert_eq!(report.final_destination, dest.join("project"));
        assert_eq!(
            std::fs::read_to_string(dest.join("project").join("src").join("main.rs"))?,
            "fn main() {}"
        );
        Ok(())
    }

    #[test]
    fn test_single_stream_gz_and_zst() -> Result<()> {
        let dir = tempdir()?;
        let gz_path = dir.path().join("notes.txt.gz");
        {
            let f = File::create(&gz_path)?;
            let mut gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            gz.write_all(b"compressed text payload")?;
            gz.finish()?;
        }
        assert_eq!(ArchiveService::detect_format(&gz_path), Some(ArchiveFormat::Gzip));
        let v_items = ArchiveService::list_virtual_directory(&gz_path, "")?;
        assert_eq!(v_items.len(), 1);
        assert_eq!(v_items[0].name, "notes.txt");

        let out_dir = dir.path().join("gz_out");
        let rep = ArchiveService::extract_archive_with_options(
            &ExtractOptions {
                archive_path: gz_path,
                destination: out_dir.clone(),
                selected_entries: None,
                create_subfolder: false,
                overwrite: true,
                password: None,
                policy: ArchiveSafetyPolicy::default(),
            },
            |_| {},
            &AtomicBool::new(false),
        )?;
        assert_eq!(rep.extracted_files, 1);
        assert_eq!(
            std::fs::read_to_string(out_dir.join("notes.txt"))?,
            "compressed text payload"
        );

        let zst_path = dir.path().join("data.bin.zst");
        {
            let f = File::create(&zst_path)?;
            let mut enc = zstd::stream::Encoder::new(f, 3)?;
            enc.write_all(b"zstd data check")?;
            enc.finish()?;
        }
        assert_eq!(ArchiveService::detect_format(&zst_path), Some(ArchiveFormat::Zstd));
        let rep_z = ArchiveService::extract_archive_with_options(
            &ExtractOptions {
                archive_path: zst_path,
                destination: out_dir.clone(),
                selected_entries: None,
                create_subfolder: false,
                overwrite: true,
                password: None,
                policy: ArchiveSafetyPolicy::default(),
            },
            |_| {},
            &AtomicBool::new(false),
        )?;
        assert_eq!(rep_z.extracted_files, 1);
        assert_eq!(std::fs::read_to_string(out_dir.join("data.bin"))?, "zstd data check");
        Ok(())
    }

    #[test]
    fn test_zip_slip_and_reserved_name_rejected() {
        let policy = ArchiveSafetyPolicy::default();
        assert!(sanitize_archive_rel_path("../../Windows/System32/cmd.exe", &policy).is_err());
        assert!(sanitize_archive_rel_path("C:\\evil.txt", &policy).is_err());
        assert!(sanitize_archive_rel_path("folder/CON.txt", &policy).is_err());
        assert_eq!(
            sanitize_archive_rel_path("./safe/dir/file.txt", &policy).unwrap(),
            "safe/dir/file.txt"
        );
    }

    #[test]
    fn test_zip_bomb_ratio_protection_blocks_high_ratio_gzip() -> Result<()> {
        let dir = tempdir()?;
        let bomb_path = dir.path().join("bomb.txt.gz");
        {
            // 16 MiB of zeros compresses to ~16 KiB (~1000:1 ratio > 100:1 threshold above 10 MiB grace)
            let f = File::create(&bomb_path)?;
            let mut gz = flate2::write::GzEncoder::new(f, flate2::Compression::best());
            let zeros = vec![0u8; 64 * 1024];
            for _ in 0..256 {
                gz.write_all(&zeros)?;
            }
            gz.finish()?;
        }

        let out_dir = dir.path().join("bomb_out");
        let res = ArchiveService::extract_archive_with_options(
            &ExtractOptions {
                archive_path: bomb_path,
                destination: out_dir,
                selected_entries: None,
                create_subfolder: false,
                overwrite: true,
                password: None,
                policy: ArchiveSafetyPolicy::default(),
            },
            |_| {},
            &AtomicBool::new(false),
        );
        assert!(res.is_err(), "Zip bomb should be aborted by ratio cap");
        let msg = res.unwrap_err().to_string().to_ascii_lowercase();
        assert!(msg.contains("zip bomb") || msg.contains("ratio"), "err was: {}", msg);
        Ok(())
    }
}
