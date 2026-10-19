# Visual Parity Run — files-rs vs Files-community (live, iterative)

> Running log. Every entry: what was tried, what worked, what didn't.
> Rule learned: never repeat a failed approach — record it here.
> Started: 2026-10-15 ~05:00 IST. Target: ≥8h, all sections.

## Sections checklist
- [ ] 1. Window backdrop (Mica/MicaAlt translucency)
- [ ] 2. Tab bar (geometry, active blending, drag/reorder)
- [ ] 3. Navigation bar / omnibar (height, buttons, breadcrumb, modes)
- [ ] 4. Toolbar / command bar (buttons, split buttons, overflow)
- [ ] 5. Sidebar (widths, rows, selection pill, tree, icons)
- [ ] 6. File views (details/list/grid/columns geometry + hover/selection)
- [ ] 7. Status bar (height, transparency, items)
- [ ] 8. Context menus (item menu, background menu vs Files flyouts)
- [ ] 9. Animations (entrance, hover, selection, flyouts, scroll physics)
- [ ] 10. Fonts & icons (Segoe Variable, Fluent Icons)

## Log

### 05:00 — §1 Mica investigation chain (transparency pipeline, root-caused)
- Tried: read current tokens (`BG_MAIN 0x1C1C1CD8` etc.), vibrancy call sites, user settings (`mica_enabled=true`).
- Tried: zeroed root/sidebar/toolbar fills to `0x00000000`, rebuilt, screenshot — still 100% opaque. Fills NOT the blocker.
- Tried: logged vibrancy result — `apply_mica applied` (DWM accepts). DWM/taskbar blur works OS-wide. Not the call.
- Tried: read gpui 0.2.2 renderer source. FOUND IT: `set_background_appearance(Blurred)` sets legacy `ACCENT_ENABLE_ACRYLICBLURBEHIND`, which fights `DWMWA_SYSTEMBACKDROP_TYPE` (Mica) — the two composition systems cancel out to opaque. Swapchain itself is alpha-capable (`PREMULTIPLIED`, DirectComposition on).
- Fix applied: `Transparent` (TRANSPARENTGRADIENT, no competing policy) + vibrancy Mica on top. Rebuilding to verify visually.
- Result: STILL opaque. Transparent didn't help either.
- Root-caused deeper: DWM spy confirms backdrop=2 (Mica) on the live HWND; swapchain is premultiplied-alpha DirectComposition; fills are genuinely translucent (`rgba()` = RRGGBBAA verified); scene root has no bg. New theory: BOTH legacy accent policies (Acrylic AND TransparentGradient) fight `DWMWA_SYSTEMBACKDROP_TYPE`. Fix applied: `Opaque` (ACCENT_DISABLED = clean slate) + vibrancy Mica + scene RGBA fills. This is the textbook recipe.
### 05:30 — §1 Mica verdict: REFERENCE ITSELF IS OPAQUE, chasing ghost
- Captured the REAL Files app (Files.exe): tab strip ~#1E1E1E, address ~#2D2D2D, sidebar ~#2A2A2A, content ~#323233 — ALL FLAT OPAQUE. Taskbar in same shot shows wallpaper blur (DWM works), Files shows none.
- Conclusion: visible translucency is NOT a differentiator. Dark-theme Mica Alt is ~opaque by design; the model's "translucent tokens" chased something the reference doesn't exhibit.
- Decision: opaque fills matching reference tokens exactly; keep vibrancy call (harmless, matters for light mode); drop transparency tuning. Geometry/typography/iconography are the real gaps.
- Reference numbers captured: tabs 32px h, 8px top radius, 4-8px gaps, active = navbar-color fill; sidebar rows 28-30px, selection = full-width #3D3D3D rounded + 2-3px blue left bar; toolbar 40-44px, blue Cut/Copy; cards 110-120x130-150, checkbox 16px top-left on hover/select, selection = 1px #60CDFF border + faint fill; label 12px, 2 lines + ellipsis.

### 05:45 — §8 Context menus (structure + motion)
- Compared canonical order vs `ContentPageContextFlyoutFactory.cs`: ours already matches (Open, Open With, New Tab/Window, Terminal, pins, clipboard, Properties).
- Live Files menu via synthetic VK_APPS: ignored, no menu opened. Structural parity asserted from XAML tokens instead (radius 8, 250px width, 480px max, separators).
- Diffs fixed: all glyphs were accent-blue (Files: monochrome secondary) → secondary; labels 13px → 14px; added 120ms opacity entrance on finished menu.
- Learned: `with_animation` returns AnimationElement (no `.child`) — must wrap at return; changed return type to `impl IntoElement`.

### 06:15 — §3 Nav bar + §5/§6 selection pills
- Nav bar 44px → 48px, buttons 32x32 → 36x32, dropped bottom divider (reference is seamless).
- Details flat/grouped + List flat/grouped: full-height accent border → 3x16 rounded pill child (matches sidebar pill + reference rows).
- Sidebar selection already conformant (translucent block + 3x16 pill); status bar 32px with counts/git/mode conformant; fonts already Variable/Fluent.

### 06:40 — Pill verification: vision lies, pixels don't (READ THIS)
- After adding pills, 4 separate vision_analyze crops all reported "no pill, tint only" — including a 4x nearest-neighbor enlargement where the model mistook the zip icon's blue spine for the only blue.
- Pixel scan (PIL, accent-blue runs >=12px vertical) found the truth: 3px-wide x 14-16px-tall ACCENT run at x=258-260, y=539-554 — exactly the selected row's left edge. The pill was rendering all along.
- RULE: never trust prose vision for elements <=4px. Verify micro-geometry with pixel scans: load PNG in PIL, threshold the exact token color, find vertical/horizontal runs, report coordinates. One 30-line script beats ten crop analyses.
- Almost "fixed" working code with a width-toggle refactor based on false vision; reverted (conditional child proven on pixels). Width-toggle also would have shifted every row's content via flex gap.
- Also fixed: flat-details pill used stray gold 0xF5B942 instead of ACCENT blue → now ACCENT_COLOR_RGB everywhere.

### 07:00 — §9 Dialog entrances
- Inventory: tooltip fade + menu fade + cursor blink existed. Added 120ms dialog-container fade (same pattern) in `dialogs.rs` render.
- Skipped (low value / high risk): tab open/close layout animation, sidebar collapse animation, hover fades (GPUI hover is stateless).

### 07:15 — §6 Grid cards
- Selection was 2px accent border, no fill → now 1px #60CDFF + faint selected fill (reference tokens).
- Checkbox only in checkbox-mode → now also on selected cards (reference shows on hover/select; hover-structure impossible in GPUI).

### 07:30 — §6 Columns view
- Rows had fill only → added 3x16 accent pill (same proven pattern; all five surfaces now unified: sidebar/details/list/grid-checkbox/columns).

### 07:35 — §1 Caption buttons: conformant, no change
- 46x32, Segoe glyphs E921/E922/E923/E8BB, red #E81123 close-hover. Matches Windows/Files spec.

### 07:50 — §3 Nav buttons, scrollbars, home/empty/preview
- Up glyph was E197 (Upload) → E74A (Up, Files token). Back E72B / Forward E72A / Refresh E72C already correct.
- Removed tracked stale `navigation_bar.rs.bak` (41KB Jun23 leftover).
- Scrollbars: custom 6px minimal (bouncy_scroll.rs) — close to Files overlay style, leave.
- Home (Quick Access/Drives/Recent/Tags/Network), empty-state, details pane (thumbnail/name/Properties/Dimensions/Dates) all structurally conformant.

### 08:05 — §8 Functional gap: keyboard context menu
- Synthetic right-click/Apps-key never reached either app (message-level limitation), which exposed a REAL gap: our app had no keyboard path to context menus at all.
- Added Menu-key (VK_APPS→"menu" in GPUI) + Shift+F10 → `show_context_menu(first_selected, viewport-center)`; threaded `&mut Window` through `handle_key_event` (single caller).

### 19 Oct — Main-view + context-menu parity vs installed Files (CUA captures)
- Method: PrintWindow black for WinUI → staged windows side-by-side, real `mouse_event` clicks, atomic hide-Hermes + CopyFromScreen captures, `vision_analyze` for enumeration + PIL for ≤4px.
- Item menu rebuilt to Files order: icon toolbar (Cut/Copy/Paste/Share/Delete/Properties) + Open / Open with › / Copy item path (Ctrl+Shift+C) / Create shortcut / Compress › / Extract Here? / Open in New Tab / New Window / sep / Pins / sep / Rename (F2) / sep / Extras (VS Code, Git Status, Delete Permanently). Removed flat Cut/Copy/Paste/Share/Delete/Properties rows (toolbar covers) and 48px primaries (now 32px rows with hints/chevrons E76C).
- New `CopyItemPath` action (Ctrl+Shift+C) wired to Ctrl+Shift+C binding + `copy_item_path` handler.
- Bg menu reordered to Files: View / Sort By / NEW Group by (None/Name/Date modified/Date created/Size/Type→FileType) / Refresh / New Folder / New Text Document / Properties / Terminal / Paste(end).
- Grid zoom curve 80+140z → 140+120z (cap 260): default 23% now renders 5× ~187px cards like Files large icons (was 8-9 narrow cols with mid-word breaks).
- Status bar left → Files text: "N items | M selected | size" (singular "1 item selected").
- FPS badge gated behind FILES_RS_FPS=1 (was a fake "tab" in the tab strip).
- Deviations kept: no submenu flyouts yet (chevron rows fire the default action — flat sections instead), Rename kept in item menu (no Show-more dialog to host it), Paste kept on bg menu, extras section, Properties on bg menu.
- Verified on pixels: item menu (toolbar + 12 rows + hints + chevrons), bg menu (View/Sort/Group sections), 5-col grid, "420 items | 1 item selected | 164.9 KB".

### 19 Oct (evening) — Files-canonical menus, shell extensions, behavior fixes
- Read Files-community `ContentPageContextFlyoutFactory.cs` (`GetBaseItemMenuItems`):
  item menu = toolbar + Open / Open with › / Paste shortcut / Copy item path /
  Create folder with selection / Create shortcut / Compress › / Send to › /
  (pins, extras) / Show more options; bg menu = Layout › / Sort by › /
  Group by › / Refresh / New › / Pin to Sidebar / Open in Terminal /
  Show more options. Rebuilt both menus to exactly this.
- Hover flyout submenus (`SubmenuKind` + `on_hover` on stateful row wrappers,
  panel child of the row so hover persists into the flyout): Layout, Sort by,
  Group by, New, Open With (Default app + Choose another app...), Compress
  (Compress to ZIP + conditional Extract Here), Send to (live %APPDATA%
  SendTo entries; folder targets copy, executables launch with paths).
- "Show more options" opens the NATIVE classic shell menu via `IContextMenu`
  on a dedicated STA thread (`shell_menu_service.rs`: SHParseDisplayName →
  SHBindToParent → BindToObject → GetUIObjectOf → QueryContextMenu →
  TrackPopupMenuEx → InvokeCommand). All third-party extensions (WinRAR,
  7-Zip, Kaspersky, ...) + verbs (Paste, Properties, Send to, sharing)
  included; works for item/multi/background. Windows-only row (cfg-gated).
- Right-click now SELECTS first (exclusive unless already in selection, so
  multi-select is preserved); right-click on empty space clears selection and
  opens the background menu. Menu open also kills any visible hover tooltip.
- Grid: card height now reserves 2 text lines from measured `text_size`
  (label can no longer spill past the selection border) + `overflow_hidden`
  on cards; labels already middle-ellipsis at 48 chars.
- Details: header Date/Type/Size columns now align with rows (rows gained the
  same 8px spacers as the in-flow resize handles: 8/16/24px drift fixed).
- Properties error fixed everywhere (3rd rundll32 PropertiesDialogW in bg
  row found too): all go through ShellExecuteExW "properties" verb.
- Refresh hint is now Ctrl+R (Ctrl+R bound in key handler; F5 still works);
  Open in Terminal hint is Ctrl+` (Ctrl+` bound too).
- Hover tooltip delay 350ms → 900ms; never appears over an open menu.
- New actions: `create_new_folder`, `create_new_text_document`,
  `create_folder_with_selection` (New folder + move selection in),
  `paste_as_shortcut` (.lnk per clipboard item via ShortcutService),
  `send_to_target`, `show_classic_menu_for`, `set_open_submenu`.
- Deferred with reason: "Edit file tags" row (tag_service exists but there is
  no tag editor dialog — needs a new dialog, say so if wanted); submenu items
  apply + close the menu (Files keeps it open with checkmarks — needs
  checkable rows); IContextMenu2 owner-draw (most extensions fine without).
- VERIFICATION STATUS: `cargo check --lib` clean, release build finished, app
  relaunched and running. machine was at the lock screen during pixel
  verification, so no input or screenshot proved the new menus yet.
### 19 Oct (night) — Show more options crashed the app: root cause + fix
- Log proved it: `Classic shell menu failed: GetUIObjectOf...` then the
  process died. Cause: `SHBindToParent`/`ILFindLastID` returns an INTERIOR
  pointer into the full PIDL; the old code freed the full PIDL and kept using
  the interior pointer (use-after-free -> GetUIObjectOf read garbage and
  failed; later CoTaskMemFree on it corrupted the heap -> delayed crash).
  Fixed with `ILClone(ILFindLastID(full))` owned copies (`shell_menu_service.rs`).
- Hovering "Show more options" showing no flyout is CORRECT Files behavior —
  it is a click row that opens the classic menu, not a submenu. Clicking now
  opens the native menu instead of crashing (fix verified by code + log so
  far; awaiting user click-confirmation since the screen is contested).
- Look pass from Files' measured pixels (292px wide, ~28px rows, ~34px
  toolbar): menu w 250->292 + 4px vertical padding, rows py 8->5 (~28px),
  glyphs 14->16, toolbar buttons 36x32->34x28 (7 buttons fit without spill),
  submenu anchor follows (left 290), viewport clamp 284->300.
### 19 Oct (night) — toolbar close, StatusCenter bell + live transfers
- Toolbar icons (Cut/Copy/Paste/Rename/Share/Delete) now close the menu when
  they fire. Rename was never broken — it was buried under the open menu.
- Notification center: bell button top-right after the omnibar with an
  in-progress count badge (red on failure). Opens a Files-style flyout:
  header + Clear completed, per-job cards with state icon, progress bar,
  speed/ETA/current-file message, expandable 88px speed graph, Cancel for
  running jobs, dismiss for finished ones.
- Transfers report byte-level progress with speed + ETA (chunked async copy,
  throttled broadcasts): paste (copy/move incl. cross-volume), permanent
  delete, and trash delete (was a log-only stub) all run as engine jobs.
  Verified headless: 5MB tree copies byte-exact with live updates to 100%.
- Bg menu now matches Files: added "Paste shortcut", "Open in Terminal" is
  "Open in Windows Terminal", separators after Refresh and after Pin to
  Sidebar (none before Show more options).
- Speed: classic items preload the moment the menu opens (not on hover), in
  two phases — labels in ~50ms so the flyout renders instantly with fallback
  glyphs, real icons filling in right after. Hovering ShowMore is now a
  cache hit, not a COM round-trip.
- Nested auto-expand fixed: the open-path test required `open` to be LONGER
  than the row path, so hovering WinRAR/Send to/Kaspersky › could never show
  children. Children now show whenever the row is on the open path.
- Third-level placement: flipped nested panels flip back right when the menu
  hugs the left edge; bottom-anchor + caps kept so nothing clips.
- Black strip: outer container squeezed the 292px menu into 280px, leaving a
  10px gap before the flyout. Outer is now exactly 292 for context menus.
- Flyout no longer dies mid-travel: leaving a row starts a 350ms grace-close
  instead of an instant close; entering any row or panel cancels it. Diagonal
  cursor paths from "Show more options ›" into the panel survive.
- Icons: verbs without shell bitmaps now get MDL2 fallbacks (Open, Pin,
  Search, Terminal, Rename, TeraCopy, Scan, Upload...) so nearly every extra
  row shows an icon like the native menu does.
- Geometry: tall classic flyouts bottom-anchor (grow UP from the row) with an
  on-screen cap + internal scroll; nested panels the same; all panels flip
  left when there is no room on the right. Menu row heights are accumulated
  at build time so caps are exact. Nothing can leave the window.
- Three styles in Settings › General › Context menu: Files (hover flyouts),
  Expanded (every classic item inlined, no hover), Windows classic (native
  popup, zero porting — third-party verbs appear as-is).
- "Show more options" is now a hover submenu like the rest: the native
  `IContextMenu` is queried into an `HMENU`, enumerated to data (labels,
  states, REAL extension icons via GetDIBits→RenderImage) and rendered as
  GPUI rows — WinRAR › (+ its nested flyout), Kaspersky, Zed, ShareX, etc.
  with their real icons, separators, disabled dimming. Clicks route back
  through `InvokeCommand` on a fresh STA query with an (offset, label) guard
  so a stale offset can never fire the wrong verb. No `TrackPopupMenuEx`
  popup anymore — nothing to crash, no focus tricks.
- Verified headless (`examples/classic_probe.rs`): 34 items for a .txt with
  the user's actual extensions, 6+ real icons, nested Send to/Open with.
  Mnemonic `&` markers stripped (Files hides them too).
- Menu tooltips: hovering toolbar icons shows "Cut — Ctrl+X" etc.; classic
  rows show their label; rendered as a floating label at the cursor while a
  menu is open. Static rows already show shortcuts as inline hints.

### 19 Oct (later) — user screenshot teardown, menu was still broken
- User screenshot proved 3 live defects my captures missed: (1) menu bg rendered
  REDDISH — `rgba()` takes 0xRRGGBBAA but menu bg was written AARRGGBB
  (`0xF21A1A1A` = red @10%); fixed to `0x1A1A1AF2`. Same latent bug fixed in
  6 more spots: unchecked-checkbox + panel-row bg (`0x12FFFFFF` → `0xFFFFFF12`,
  was opaque pale-cyan), zebra striping in all 4 views (`0x0A000000` →
  `0x0000000A`, was fully transparent). Rule: all hex colors are RRGGBBAA.
- (2) Row icon/tofu boxes — `context_menu_row` + `context_menu_item` icon divs
  never set the icon font; added `FONT_MDL2` ("Segoe Fluent Icons") to both.
  Same miss fixed on fallback file/folder glyphs in details (flat+grouped),
  list (both), columns (both). Grid + home + preview already had it.
- (3) Extras spilling past the menu box — container had `max_h(480)` while
  content is ~540px. Removed the cap; `show_context_menu` now takes `window`
  and flips the menu up/left (`window.viewport_size()`, est. 580 item / 720 bg)
  so it never leaves the viewport. All 11 call sites updated.
- Menu variants: `DialogType::ContextMenu` + builder now carry
  `selected_count`/`is_dir`. Multi-select hides Open / Open With / New Tab /
  New Window / Create Shortcut; Rename opens BulkRename for multi; Open With
  hidden for folders. Extract Here was already archive-gated.
- Empty-space left-click now clears selection in grid/details/list (details +
  list still start rubber-band lasso on drag).
- Status bar back to "8 folders, 413 files | 1 item selected | 164.9 KB".
- Grid labels: `overflow_hidden` + `shorten_filename_middle(48)` so long names
  can't bleed into neighbouring cards.
- Keyboard audit: bindings complete (Enter/Backspace/Space/F2/Delete/
  Shift+Delete/Ctrl+C-X-V-A/F5/Ctrl+T-W-N/Ctrl+Tab+digits/Menu/Shift+F10/
  Alt+arrows). Verified working incl. new Ctrl+Shift+C.
