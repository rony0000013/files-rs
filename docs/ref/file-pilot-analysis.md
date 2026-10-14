# File Pilot — Reverse-Engineering Reference for files-rs

> Goal: understand how File Pilot (Voidstar, by Vjekoslav) achieves its visual
> quality and speed, so files-rs can adopt the same techniques.
> Method: static analysis of the shipped binary (REA CLI + dependency-free PE
> probe + full string harvest). No disassembler was available; function-level
> internals are marked as unknowns in §12.
> Date: 2026-10-15. Analyst: Hermes Agent (`binary-reverse-engineering` skill).

## 1. Target identity (observations)

| Fact | Value |
|---|---|
| Installer | `C:\Users\rony0\Downloads\FPilot.exe` (2,585,680 bytes) |
| Installed exe | `%LocalAppData%\Voidstar\FilePilot\FPilot.exe` (identical size — installer copies itself) |
| SHA-256 | `ab5e7bbfa50872225f24856697ef7341a58e51a5e521435ba0dfeeb8ab3b2676` |
| Format | PE32+ x86-64, 5 sections (`.text` 2.19 MB, `.rdata`, `.data`, `.pdata`, `.rsrc`), GUI subsystem, signed (cert dir present), not packed |
| Toolchain | MSVC 14.32 (VS2022), Rich header present. **Not** .NET (no CLI header — REA `ev_93cb2e2d`), **not** Go (`ev_f9a43991`), **not** Rust (`ev_b289bf09`), **not** Electron (no ASAR, no node/chromium strings) |
| Authorship | PDB path baked in: `C:\Users\Vjekoslav\Projects\Voidstar\File Pilot\` — sources: `Source\Program\Program.c`, `Source\Program\Voidstar.h`, `Source\Win\WinMain.c`, `Source\Win\WinFontRasterizer.cpp` |
| Sidecars | None. Single exe. Config/session state = plain JSON in `%AppData%\Voidstar\FilePilot\` (`FPilot-Config.json`, `FPilot-UserData.json`) and `%LocalAppData%\Voidstar\FilePilot\FPilot-Session.json` |
| Import surface | **269 functions from 17 DLLs** (full list in `scratch/pe_probe.py FULL=1` output). No framework DLLs at all. |

## 2. Architecture at a glance

```
WinMain.c: one Win32 window (CreateWindowExW, PeekMessageW loop)
   │
   ├─ Renderer: custom D3D11 (D3D11CreateDevice + D3DCompile at runtime)
   │            or OpenGL fallback (wgl). Everything = textured quads
   │            from atlases. Rounded corners/gradients/dashes in-shader.
   ├─ Commands: 319 HK_* commands (dispatch table; every UI action is a
   │            command → keymap, palette HK_CommandsOpen, macros all free)
   ├─ FS layer: NtQueryDirectoryFile / FindFirstFileExW + overlapped I/O
   │            + ReadDirectoryChangesW; Windows threadpool for background work
   ├─ Assets: system imagelist → GPU atlas; shell-provided thumbnails;
   │            single bounded cache (AssetCacheSize, default 1024)
   └─ State: JSON config/session/userdata, mutex-guarded, version-tolerant
```

The mental model is a **game engine, not a desktop app**: camera (`HK_ScrollCenterCamera`),
cursor decoupled from selection (`HK_ScrollMoveCursor*` vs `HK_ScrollSelect*`),
texture atlases, SDF shapes, VSync setting, `QueryPerformanceCounter` frame timing.

## 3. Renderer (the core speed secret)

- **One D3D11 device, runtime-compiled shaders.** Imports are only
  `D3D11CreateDevice` (d3d11.dll) + `D3DCompile` (D3DCOMPILER_47.dll). No D2D,
  no DirectComposition, no DXGI imports — swapchain handles are fetched via
  `GetProcAddress` or COM, keeping the import table (and loader work) minimal.
  OpenGL fallback uses only GL 1.x entry points (`glTexParameteri`,
  `glViewport`, `wglCreateContext`, …) — same abstract renderer, two backends,
  switched by `GraphicsBackend` (`Setting_EnumValueDirect3D/OpenGL`, offsets
  `0x221ea48+`). GLSL (`uniform sampler2DArray`, `0x22c713`) and HLSL
  (`cbuffer constants : register(b0)`, `0x22e760`) sources are **embedded as
  strings** — patchable without recompiling.
- **Texture2DArray atlas.** Pixel shader (`0x22f8f0`): single
  `textureSampler.Sample(sampler0, float3(uv, depth))` — all icons, glyphs,
  thumbnails live in array slices. Mono path reuses the red channel
  (`float4(sampled.r,…)` at `0x22fa18`) — glyphs cost 1 byte/px.
- **Vector UI in-shader.** `RoundedBoxSDF` (`0x22f4d7`), per-corner radii
  (`TEXCOORD3`), thickness mask (`TEXCOORD4`), `IsDashedOutline`, 4-corner
  gradients (`COLOR1` = TL/TR/BR/BL). Rounded corners/hover rings/focus dashes
  = zero extra geometry, zero raster cost. `RoundCorners`/`Animations` are just
  settings toggles on this pipeline.
- **Text = quads.** `WinFontRasterizer.cpp` + `CWin__FontCreateAtlas` /
  `IWin_FontCreateAtlasFromBytes` (`0x233aff`). DirectWrite is used once
  (`DWriteCreateFactory`) — almost certainly glyph raster → atlas upload, then
  layout/draw is all internal (config even names the font *file*:
  `"FontName": "segoeui.ttf"`). No per-frame text layout through the OS.

files-rs transfer: GPU atlas for icons+glyphs+thumbs; SDF rounded rects;
runtime-compiled shaders; never let the OS lay out text per frame.

## 4. Input & command system

- **319 `HK_*` commands** harvested (`scratch/fp-ascii.txt`, `grep -o "HK_[A-Za-z0-9]*" | sort -u`).
  Families: `Selection*` (copy/cut/paste/duplicate/trash/restore/as-path),
  `Scroll*` (select vs cursor vs extend, vim `HJKL` + `Ctrl+D/U`, `ZZ` centers
  camera), `Panel*` (splits, viewports, filters), `Tab*` (incl. reopen/duplicate),
  `Text*` (full modal editor: undo/redo/autocomplete/case), `GroupBy*`/`SortBy*`
  (~20 keys each), `Inspector*` (image scale/grid/minimap, line numbers, wrap),
  `Rename*` (date/incremental-ID insertion), popups (Files/Types/GoTo/QuickAccess).
- **Cursor ≠ selection** is the standout design: a game-style cursor moves
  independently; selection is an operation on top. Enables keyboard speed without
  disturbing multi-select state.
- **Perceived latency:** `"ActOnPress": true` — activates on mouse-down, saving
  ~100 ms vs click-release. `"CharacterKeyAction": "TypeInFilter"` — typing never
  waits. `HK_TabReopen`, `HK_SelectionCopyNextPanel` etc. show every flow kept
  on-keyboard.
- UI text is keyed (`Tooltip_*`, `PanelInspector_Label*`, `HK_*_Tooltip`) — a
  string table, trivially localizable (cf. files-rs `locales/`).

## 5. Filesystem layer

Imports (kernel32, all observed): `NtQueryDirectoryFile` (fast path, bypasses
FindFirstFile overhead), `FindFirstFileExW` (+`FindNextFileW`), overlapped I/O
(`CancelIoEx`, `GetOverlappedResult`, `DeviceIoControl` — cancellable directory
reads), `ReadDirectoryChangesW` with explicit failure logging (`'%S' - Initial /
Subsequent ReadDirectoryChangesW failed, error %lu`), `GetFinalPathNameByHandleW`
(long paths — matches `"LongPathSupport": true`), volume APIs
(`GetDiskFreeSpaceExW`, `FindFirstVolumeW`, `GetVolumeInformationW`).
Shell namespace via `SHParseDisplayName`/`SHBindToParent`/`Win_GetIShellFolder`,
known folders, `SHGetFileInfoW`, property system (`PSGetPropertyDescription`,
`SHGetPropertyStoreFromParsingName` — inspector metadata without opening files).
Network: `NetShareEnum` + `WNetGetConnectionW`.
UI pump: `MsgWaitForMultipleObjectsEx` — directory-change events wake the same
thread that pumps messages; background work via `CreateThreadpool*` /
`SubmitThreadpoolWork` (measured: **~74 threads** under load).

Costly work is conditional: `CalcFolderSizes{SSD:true, HDD:false, Network:false}` —
it knows spinning disks stall. `LoadThumbnails: LocalDisk` — never from network.

## 6. Icons, thumbnails, asset cache

- Icons: `Win_GetIconFromImageList` (`0x231f20`) — **one shared system imagelist
  handle, uploaded into the GPU atlas once**, tinted in-shader (`Color_IconTint`).
- No image codecs linked (no WIC, no GDI+, no stb markers in strings).
  Thumbnails therefore come from the **shell thumbnail cache/providers** via COM
  (`CoCreateInstance`, `RegisterDragDrop`/`RevokeDragDrop` for DnD) and are
  uploaded with `CreateDIBSection` (`0x2449fc`) → atlas slice. Zero decoder
  maintenance, zero decoder CVEs, Windows-cache-speed.
- One bounded cache for everything: *"Defines the maximum memory used for icons,
  thumbnails, and previews"* (`0x21b220`), `AssetCacheSize` default 1024.
- Inspector reads headers directly (dimensions, line count, encoding labels
  `Inspector_TextEncodingUTF8`, image scale fit/original/custom + minimap) — no
  full-file parse for previews.

## 7. Startup & state

Cold→window **577 ms**, warm **405 ms** (measured 2026-10-15, `scratch/fpilot_measure.ps1`;
method: `Start-Process` + poll `MainWindowHandle`, 5 s settle, graceful `taskkill`,
state files backed up + restored). Steady working set **~405 MB** with 4 panels
restoring + thumbnails — almost all bounded asset cache, not code (`.text` 2.2 MB).
Why sub-second: no runtime/CLR/Chromium init, no XAML parse, 269 imports to bind,
JSON state loads in ms, heavy work (thumbs, folder sizes) deferred to threadpool
*after* first paint. `"LoadOnStartup": "LastSession"` restores tabs/panels/columns
from `FPilot-Session.json` (per-panel path, zoom, sort, column ratios).

Crash hygiene: `SetUnhandledExceptionFilter` + `%S\FPilot%S_%S%S_%04d-...dmp`
minidumps, per-file mutexes (`FilePilot_Config_Mutex`…), version-tolerant JSON
(`"…version %1… from an older format"`), `Process32FirstW/NextW` (single-instance
check), shell integration via registry (`OpenInFilePilot`, `FilePilotHere`).

## 8. Settings inventory (all `Setting_*`, observed)

`GraphicsBackend` (Direct3D/OpenGL), `VSync`, `AssetCacheSize`, `LoadThumbnails`
(LocalDisk), `StoreRecents`/`ShowRecents`/`SortRecents`, `CalcFolderSizes{,SSD,HDD,Network}`,
`FontName/Size`, `InspectorFontName/Size/Layout`, `Spacing`, `Animations`,
`RoundCorners`, `ColorScheme` (+Dark/Light, `SystemColorScheme`), `AlternatingRowColors`,
`InactiveOpacity`, `ShowFileExtension/HiddenFiles/SystemFiles/Breadcrumbs/Checkboxes/Tooltips`,
`ShortenedPaths`, `LongPathSupport`, `SingleClick{,FoldersOnly}`, `HoverToSelect`,
`UnderlineOnHover`, `DoubleClickToParent`, `Overscroll`, `ReverseScroll`,
`CharacterKeyAction` (TypeInFilter/SelectTypedItem), `LoadOnStartup` (LastSession),
`NewTabShow`. Full keyed defaults + tooltips live in `.rdata` (`0x21b220+`).

## 9. Evidence & reproduction pointers

- REA CLI 6.5.0: `ev_f9a43991` (Go-negative), `ev_b289bf09` (Rust-negative),
  `ev_93cb2e2d` (not-managed), `ev_666ce74b` (PE resources). Full JSON:
  `scratch/fpilot-pe.json`.
- Import table + string mining: `scratch/pe_probe.py` (stdlib-only; `FULL=1`
  dumps all 269 imports), `scratch/dump_strings.py` →
  `scratch/fp-ascii.txt` (9,266) + `scratch/fp-utf16.txt` (119).
- Shader sources: HLSL `0x22e760–0x230xxx`, GLSL `0x22c713+`. Command table:
  `HK_*` ≈ `0x22xxx` (`HK_SortByName` at `0x22db0`). Tooltip/config table from
  `0x21b220`. PDB/source paths `0x24xxxx` region (misc harvest).
- Live numbers: `scratch/fpilot_measure.ps1` (rerunnable; backs up + restores
  `%AppData%\Voidstar\FilePilot\*.json`).

## 10. files-rs transfer list (concrete)

1. **Act on mouse-down** (`ActOnPress`) — cheapest perceived-latency win; do in `src/ui/` input handling.
2. **Cursor/selection split** — game-style cursor independent of selection; enables vim flow without breaking multiselect.
3. **Atlas everything** (icons, glyphs, thumbs in one texture array; mono R-channel for text) — kills per-item upload cost during scroll.
4. **SDF rounded rects + in-shader dashes/gradients** — visual polish at zero geometry cost; maps to files-rs Fluent token work.
5. **Overlapped cancellable enumeration** (`CancelIoEx`-style; in Rust: async readdir + generation counters) with `ReadDirectoryChangesW` push updates on the message-pump thread.
6. **Conditional heavy work**: folder sizes + thumbnails only on SSD/local; never network. Detect drive type first.
7. **Single bounded asset cache** with one MB cap instead of per-subsystem caches.
8. **Shell thumbnail cache instead of bundled decoders** — less code, fewer CVEs.
9. **Defer everything past first paint**; restore session from cheap JSON; target <600 ms cold-to-window (File Pilot: 577/405 ms).
10. **Command-table architecture**: every action a named command → keymap, palette, and scripting come free; string-table UI text for locales.

## 11. Explicit unknowns (need Ghidra or runtime tracing)

Enumeration batch sizes, atlas eviction policy, sort implementation, thumbnail
request coalescing during fast scroll, frame-pacing/VSync logic, crash-dump
trigger conditions, exact `ord#727/155/88` shell helpers, image-list change
invalidation. Native disassembly requires `GHIDRA_INSTALL_DIR` (Ghidra 12.1.x) —
then `open_binary` + `analyze_function` per the `binary-reverse-engineering` skill.
