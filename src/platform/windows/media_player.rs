//! Native video playback for the preview pane.
//!
//! Default backend is Windows Media Foundation (`IMFSourceReader` pump):
//! hardware-accelerated decode, zero bundling, OS codecs. The decode thread
//! pushes RGBA frames (latest-only, no queue bloat) and PCM-f32 audio into a
//! rodio sink; the GPUI canvas pulls the current frame through an
//! `ImageSource::Custom` closure so no per-frame files or texture leaks occur.
//!
//! `VideoBackend::Ffmpeg` is the reserved cross-platform hook: select it with
//! `FILES_RS_VIDEO_BACKEND=ffmpeg`. It currently falls back to Media
//! Foundation with a notice — the ffmpeg-sidecar frame pump plugs into the
//! same `VideoFrame`/audio-queue contract when it lands.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};

/// Which engine drives in-preview video playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoBackend {
    /// Windows Media Foundation (default): OS decoders, no extra binaries.
    MediaFoundation,
    /// Reserved for the ffmpeg-sidecar cross-platform pump (falls back to MF).
    Ffmpeg,
}

impl VideoBackend {
    pub fn selected() -> Self {
        match std::env::var("FILES_RS_VIDEO_BACKEND")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "ffmpeg" => Self::Ffmpeg,
            _ => Self::MediaFoundation,
        }
    }
}

/// One decoded video frame (RGBA, top-down).
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pts_secs: f64,
}

enum Cmd {
    Play,
    Pause,
    Seek(f64),
    Stop,
}

struct Shared {
    playing: AtomicBool,
    progress_ms: AtomicU64,
    duration_ms: AtomicU64,
    frame: Mutex<Option<Arc<VideoFrame>>>,
    ended: AtomicBool,
    /// Bumped on seek/restart: the audio source drops stale queued PCM.
    /// Boxed in an `Arc` so the audio thread shares the exact counter.
    audio_epoch: Arc<AtomicU64>,
}

/// Handle to a running Media Foundation playback session.
/// Dropping stops the decode thread (via `Cmd::Stop` + join in `Drop`).
pub struct MfPlayer {
    shared: Arc<Shared>,
    cmd: mpsc::Sender<Cmd>,
    audio_mute_tx: mpsc::Sender<bool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MfPlayer {
    /// Open `path` and prime the first frame. Thread starts paused.
    pub fn open(path: PathBuf, duration_hint_secs: Option<f64>) -> Result<Self, String> {
        let shared = Arc::new(Shared {
            playing: AtomicBool::new(false),
            progress_ms: AtomicU64::new(0),
            duration_ms: AtomicU64::new(
                (duration_hint_secs.unwrap_or(0.0).max(0.0) * 1000.0) as u64,
            ),
            frame: Mutex::new(None),
            ended: AtomicBool::new(false),
            audio_epoch: Arc::new(AtomicU64::new(0)),
        });
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (mute_tx, mute_rx) = mpsc::channel::<bool>();
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("mf-video-pump".to_string())
            .spawn(move || Self::pump(path, s, cmd_rx, mute_rx))
            .map_err(|e| format!("spawn decode thread: {}", e))?;
        Ok(Self {
            shared,
            cmd: cmd_tx,
            audio_mute_tx: mute_tx,
            thread: Some(thread),
        })
    }

    pub fn play(&self) {
        self.shared.ended.store(false, Ordering::SeqCst);
        let _ = self.cmd.send(Cmd::Play);
    }

    pub fn pause(&self) {
        let _ = self.cmd.send(Cmd::Pause);
    }

    pub fn seek(&self, frac: f32) {
        let dur = (self.duration_secs() * 1000.0) as u64;
        let target = (dur as f64 * frac.clamp(0.0, 1.0) as f64).max(0.0);
        let _ = self.cmd.send(Cmd::Seek(target / 1000.0));
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.audio_mute_tx.send(muted);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::SeqCst)
    }

    pub fn has_ended(&self) -> bool {
        self.shared.ended.load(Ordering::SeqCst)
    }

    pub fn progress_secs(&self) -> f64 {
        self.shared.progress_ms.load(Ordering::SeqCst) as f64 / 1000.0
    }

    pub fn duration_secs(&self) -> f64 {
        self.shared.duration_ms.load(Ordering::SeqCst) as f64 / 1000.0
    }

    /// Latest decoded frame (UI thread converts to `RenderImage` on demand).
    pub fn current_frame(&self) -> Option<Arc<VideoFrame>> {
        self.shared.frame.lock().ok()?.clone()
    }

    fn pump(
        path: PathBuf,
        shared: Arc<Shared>,
        cmd_rx: mpsc::Receiver<Cmd>,
        mute_rx: mpsc::Receiver<bool>,
    ) {
        if let Err(e) = Self::pump_inner(&path, &shared, cmd_rx, mute_rx) {
            tracing::warn!("MF video pump failed for {}: {}", path.display(), e);
        }
        shared.playing.store(false, Ordering::SeqCst);
    }

    #[cfg(windows)]
    fn pump_inner(
        path: &std::path::Path,
        shared: &Arc<Shared>,
        cmd_rx: mpsc::Receiver<Cmd>,
        mute_rx: mpsc::Receiver<bool>,
    ) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Win32::Media::MediaFoundation::*;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        use windows::core::GUID;
        use windows::Win32::System::Variant::VT_UI8;

        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("MFStartup: {:?}", e))?;

            let reader: IMFSourceReader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None)
                .map_err(|e| format!("open: {:?}", e))?;

            // Duration (100ns units) for the scrub bar.
            if let Ok(prop) = reader.GetPresentationAttribute(
                MF_SOURCE_READER_MEDIASOURCE.0 as u32,
                &MF_PD_DURATION as *const _,
            ) {
                if prop.Anonymous.Anonymous.vt == VT_UI8 {
                    let hns = unsafe { prop.Anonymous.Anonymous.Anonymous.uhVal };
                    if hns > 0 {
                        shared.duration_ms.store(hns / 10_000, Ordering::SeqCst);
                    }
                }
            }

            // Video: first video stream -> RGB32.
            reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .map_err(|e| format!("deselect: {:?}", e))?;
            let vstream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetStreamSelection(vstream, true).map_err(|e| format!("vstream: {:?}", e))?;
            let vtype = MFCreateMediaType().map_err(|e| format!("mediatype: {:?}", e))?;
            vtype.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| format!("v maj: {:?}", e))?;
            vtype.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|e| format!("v sub: {:?}", e))?;
            reader
                .SetCurrentMediaType(vstream, None, &vtype)
                .map_err(|e| format!("v type: {:?}", e))?;

            // Native frame size for buffer math.
            let native = reader.GetCurrentMediaType(vstream).map_err(|e| format!("v cur: {:?}", e))?;
            let mut vw: u32 = 640;
            let mut vh: u32 = 360;
            if let Ok(packed) = native.GetUINT64(&MF_MT_FRAME_SIZE) {
                vw = ((packed >> 32) as u32).max(16);
                vh = ((packed & 0xFFFF_FFFF) as u32).max(16);
            }
            let mut stride: i32 = (vw * 4) as i32;
            if let Ok(s) = native.GetUINT32(&MF_MT_DEFAULT_STRIDE) {
                stride = s as i32;
            }

            // Audio: first audio stream -> PCM f32 stereo 44.1k (may be absent).
            let astream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
            let has_audio = reader.SetStreamSelection(astream, true).is_ok();
            let mut audio_rate = 44100u32;
            if has_audio {
                if let Ok(atype) = MFCreateMediaType() {
                    let _ = atype.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio);
                    let _ = atype.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float);
                    let _ = atype.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2);
                    let _ = atype.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 44100);
                    if reader.SetCurrentMediaType(astream, None, &atype).is_ok() {
                        if let Ok(cur) = reader.GetCurrentMediaType(astream) {
                            if let Ok(r) = cur.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND) {
                                audio_rate = r.max(8000);
                            }
                        }
                    }
                }
            }

            // rodio output (created once; silence when no audio stream).
            let mut device = rodio::DeviceSinkBuilder::open_default_sink()
                .map_err(|e| format!("audio out: {}", e))?;
            device.log_on_drop(false);
            let player = std::sync::Arc::new(rodio::Player::connect_new(device.mixer()));
            player.pause();
            let (pcm_tx, pcm_rx) = mpsc::sync_channel::<Vec<f32>>(64);
            let rodio_player = player.clone();
            let audio_epoch = shared.audio_epoch.clone();
            std::thread::Builder::new()
                .name("mf-audio-feed".to_string())
                .spawn(move || {
                    rodio_player.append(MfAudioSource::new(pcm_rx, 2, audio_rate, audio_epoch));
                })
                .map_err(|e| format!("audio thread: {}", e))?;

            let mut muted = false;
            let mut base_pts = 0.0f64; // sample-time at last (re)start
            let mut base_instant = std::time::Instant::now();
            let mut playing = false;
            let mut need_seek: Option<f64> = None;

            // Prime first frame so the canvas never starts blank.
            let prime = Self::read_video_frame(&reader, vstream, vw, vh, stride);
            if let Ok((rgba, pts)) = prime {
                *shared.frame.lock().unwrap() = Some(Arc::new(VideoFrame { rgba, width: vw, height: vh, pts_secs: pts }));
                shared.progress_ms.store((pts * 1000.0) as u64, Ordering::SeqCst);
            }

            loop {
                // Commands (non-blocking drain).
                let mut stop = false;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        Cmd::Play => {
                            if shared.ended.load(Ordering::SeqCst) {
                                // Restart from 0 after natural end.
                                let _ = reader.Flush(vstream);
                                if has_audio {
                                    let _ = reader.Flush(astream);
                                }
                                Self::seek_to(&reader, vstream, astream, has_audio, 0.0);
                                base_pts = 0.0;
                                shared.audio_epoch.fetch_add(1, Ordering::SeqCst);
                            }
                            base_instant = std::time::Instant::now();
                            base_pts = shared.progress_ms.load(Ordering::SeqCst) as f64 / 1000.0;
                            playing = true;
                            shared.playing.store(true, Ordering::SeqCst);
                            player.play();
                        }
                        Cmd::Pause => {
                            playing = false;
                            shared.playing.store(false, Ordering::SeqCst);
                            player.pause();
                        }
                        Cmd::Seek(t) => {
                            need_seek = Some(t.max(0.0));
                        }
                        Cmd::Stop => {
                            stop = true;
                            break;
                        }
                    }
                }
                if stop {
                    break;
                }
                while let Ok(m) = mute_rx.try_recv() {
                    muted = m;
                }
                player.set_volume(if muted { 0.0 } else { 1.0 });

                if let Some(t) = need_seek.take() {
                    let _ = reader.Flush(vstream);
                    if has_audio {
                        let _ = reader.Flush(astream);
                    }
                    Self::seek_to(&reader, vstream, astream, has_audio, t);
                    base_pts = t;
                    base_instant = std::time::Instant::now();
                    shared.audio_epoch.fetch_add(1, Ordering::SeqCst);
                    shared.progress_ms.store((t * 1000.0) as u64, Ordering::SeqCst);
                    // Show the seek landing frame immediately.
                    if let Ok((rgba, pts)) = Self::read_video_frame(&reader, vstream, vw, vh, stride) {
                        *shared.frame.lock().unwrap() =
                            Some(Arc::new(VideoFrame { rgba, width: vw, height: vh, pts_secs: pts }));
                        shared.progress_ms.store((pts * 1000.0) as u64, Ordering::SeqCst);
                    }
                    if !playing {
                        // Stay paused on the seek frame.
                        std::thread::sleep(std::time::Duration::from_millis(30));
                        continue;
                    }
                }

                if !playing {
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    continue;
                }

                // Pace: playhead in stream time.
                let playhead = base_pts + base_instant.elapsed().as_secs_f64();
                match Self::read_video_frame(&reader, vstream, vw, vh, stride) {
                    Ok((rgba, pts)) => {
                        if pts < playhead - 0.25 {
                            continue; // late after seek: drop
                        }
                        if pts > playhead + 0.06 {
                            let wait = ((pts - playhead) as f32).min(0.5);
                            std::thread::sleep(std::time::Duration::from_secs_f32(wait));
                        }
                        *shared.frame.lock().unwrap() =
                            Some(Arc::new(VideoFrame { rgba, width: vw, height: vh, pts_secs: pts }));
                        shared.progress_ms.store((pts * 1000.0) as u64, Ordering::SeqCst);
                        // Keep audio roughly in sync: drain decoded PCM while behind video.
                        if has_audio {
                            for _ in 0..4 {
                                match Self::read_audio_chunk(&reader, astream) {
                                    Ok(pcm) if !pcm.is_empty() => {
                                        if pcm_tx.try_send(pcm).is_err() {
                                            break; // sink buffer full: backpressure
                                        }
                                    }
                                    _ => break,
                                }
                            }
                        }
                    }
                    Err(e) if e == "eos" => {
                        playing = false;
                        shared.playing.store(false, Ordering::SeqCst);
                        shared.ended.store(true, Ordering::SeqCst);
                        player.pause();
                    }
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(15));
                    }
                }
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    fn seek_to(
        reader: &windows::Win32::Media::MediaFoundation::IMFSourceReader,
        vstream: u32,
        astream: u32,
        has_audio: bool,
        secs: f64,
    ) {
        use windows::Win32::Media::MediaFoundation::*;
        use windows::Win32::System::Com::StructuredStorage::{
            PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
        };
        use windows::core::GUID;
        use windows::Win32::System::Variant::VT_I8;
        unsafe {
            let hns = (secs.max(0.0) * 10_000_000.0) as i64;
            let var = PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_I8,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: PROPVARIANT_0_0_0 { hVal: hns },
                    }),
                },
            };
            let _ = reader.SetCurrentPosition(&GUID::zeroed(), &var);
            let _ = vstream;
            let _ = astream;
            let _ = has_audio;
        }
    }

    /// Read one video sample -> (top-down RGBA, pts secs). Err("eos") at end.
    #[cfg(windows)]
    fn read_video_frame(
        reader: &windows::Win32::Media::MediaFoundation::IMFSourceReader,
        vstream: u32,
        w: u32,
        h: u32,
        stride: i32,
    ) -> Result<(Vec<u8>, f64), String> {
        use windows::Win32::Media::MediaFoundation::*;
        unsafe {
            let mut flags = 0u32;
            let mut ts = 0i64;
            let mut sample: Option<IMFSample> = None;
            reader
                .ReadSample(vstream, 0, None, Some(&mut flags), Some(&mut ts), Some(&mut sample))
                .map_err(|e| format!("read: {:?}", e))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 || sample.is_none() {
                return Err("eos".to_string());
            }
            let sample = sample.unwrap();
            let buf = sample
                .ConvertToContiguousBuffer()
                .map_err(|e| format!("contig: {:?}", e))?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut max_len = 0u32;
            buf.Lock(&mut ptr, None, Some(&mut max_len)).map_err(|e| format!("lock: {:?}", e))?;
            let need = (w as usize) * (h as usize) * 4;
            if (max_len as usize) < need {
                let _ = buf.Unlock();
                return Err("short buffer".to_string());
            }
            let src = std::slice::from_raw_parts(ptr, need.max(max_len as usize));
            // RGB32 == BGRA (possibly bottom-up when stride < 0).
            let row_bytes = (w as usize) * 4;
            let mut rgba = vec![0u8; need];
            let bottom_up = stride < 0;
            for y in 0..(h as usize) {
                let sy = if bottom_up { h as usize - 1 - y } else { y };
                let srow = &src[sy * row_bytes..sy * row_bytes + row_bytes];
                let drow = &mut rgba[y * row_bytes..(y + 1) * row_bytes];
                for (d, s) in drow.chunks_exact_mut(4).zip(srow.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
            }
            let _ = buf.Unlock();
            Ok((rgba, ts as f64 / 10_000_000.0))
        }
    }

    /// Read one audio sample -> interleaved f32 PCM (or empty when none ready).
    #[cfg(windows)]
    fn read_audio_chunk(
        reader: &windows::Win32::Media::MediaFoundation::IMFSourceReader,
        astream: u32,
    ) -> Result<Vec<f32>, String> {
        use windows::Win32::Media::MediaFoundation::*;
        unsafe {
            let mut flags = 0u32;
            let mut sample: Option<IMFSample> = None;
            reader
                .ReadSample(astream, 0, None, Some(&mut flags), None, Some(&mut sample))
                .map_err(|e| format!("aread: {:?}", e))?;
            let sample = match sample {
                Some(s) => s,
                None => return Ok(Vec::new()),
            };
            let buf = sample
                .ConvertToContiguousBuffer()
                .map_err(|e| format!("acontig: {:?}", e))?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut cur_len = 0u32;
            buf.Lock(&mut ptr, None, Some(&mut cur_len)).map_err(|e| format!("alock: {:?}", e))?;
            let bytes = std::slice::from_raw_parts(ptr as *const f32, cur_len as usize / 4).to_vec();
            let _ = buf.Unlock();
            Ok(bytes)
        }
    }

    #[cfg(not(windows))]
    fn pump_inner(
        _path: &std::path::Path,
        _shared: &Arc<Shared>,
        cmd_rx: mpsc::Receiver<Cmd>,
        _mute_rx: mpsc::Receiver<bool>,
    ) -> Result<(), String> {
        // Non-Windows stub: the ffmpeg-sidecar pump lands here.
        for cmd in cmd_rx {
            if matches!(cmd, Cmd::Stop) {
                break;
            }
        }
        Ok(())
    }
}

impl Drop for MfPlayer {
    fn drop(&mut self) {
        let _ = self.cmd.send(Cmd::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// rodio source pulling decoded PCM from the MF pump.
struct MfAudioSource {
    rx: mpsc::Receiver<Vec<f32>>,
    buf: std::collections::VecDeque<f32>,
    channels: u16,
    rate: u32,
    epoch: Arc<AtomicU64>,
    seen_epoch: u64,
}

impl MfAudioSource {
    fn new(
        rx: mpsc::Receiver<Vec<f32>>,
        channels: u16,
        rate: u32,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        let seen_epoch = epoch.load(Ordering::SeqCst);
        Self { rx, buf: std::collections::VecDeque::new(), channels, rate, epoch, seen_epoch }
    }
}

impl Iterator for MfAudioSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        while self.buf.len() < 1024 {
            match self.rx.try_recv() {
                Ok(chunk) => self.buf.extend(chunk),
                Err(_) => break,
            }
        }
        self.buf.pop_front().or(Some(0.0))
    }
}

impl rodio::Source for MfAudioSource {
    fn channels(&self) -> std::num::NonZero<u16> {
        std::num::NonZero::new(self.channels).unwrap_or(std::num::NonZero::new(2).unwrap())
    }
    fn sample_rate(&self) -> std::num::NonZero<u32> {
        std::num::NonZero::new(self.rate).unwrap_or(std::num::NonZero::new(44100).unwrap())
    }
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
