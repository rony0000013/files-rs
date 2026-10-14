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
