# FlickCut — Tauri v2 Packaging & Branding Spike

## 1. Config approach per platform

One `src-tauri/tauri.conf.json` (schema `https://schema.tauri.app/config/2`)
drives all three platforms. `bundle.targets: "all"` builds every installer
native to the machine running `tauri build` (CI builds per-OS in parallel).

| Platform | Targets in draft | Notes |
|---|---|---|
| Windows | `nsis` (+`msi` optional) | `nsis.installMode: "both"` = per-user and per-machine install. NSIS gives the familiar `FlickCut_x.y.z_x64-setup.exe`. |
| macOS | `dmg` (+`app`) | `minimumSystemVersion: "10.15"`. DMG is drag-and-drop; unsigned builds trigger Gatekeeper — needs Developer ID + notarization for public release (separate task). |
| Linux | `appimage`, `deb` (`rpm` optional) | `appimage.bundleMediaFramework: true` recommended — a screen recorder will need codecs. |

`tauri build --bundles <name>` overrides targets for one-off builds.
Platform-specific overrides can live in `tauri.windows.conf.json` /
`tauri.macos.conf.json` / `tauri.linux.conf.json` (auto-merged).

## 2. WebView2 (Windows) — the one runtime gotcha

Tauri renders UI in the OS webview. macOS (WKWebView) and Linux (WebKitGTK)
ship one; **Windows needs the Edge WebView2 runtime**. The draft sets:

```json
"webviewInstallMode": { "type": "downloadBootstrapper", "silent": true }
```

The installer silently downloads the tiny WebView2 bootstrapper on demand —
installer stays small, works on any Win10/11 machine. For fully offline
installs use `"type": "embedBootstrapper"` (adds ~2 MB). Most end-user
machines already have WebView2 (it ships with Windows 11 and Edge).

## 3. Icons — what the transparent logo needs (verified)

`tauri icon` takes **one squared PNG or SVG with transparency**
(default `./app-icon.png`) and generates everything into `src-tauri/icons/`.

I ran the real pipeline (`@tauri-apps/cli`, `npx tauri icon`) on our asset:

- **Source used:** `icons/icon-transparent.png` — the square 952x952 F-mark
  with transparency. This is the correct source. The full lockup
  (`logo-transparent.png`, 1296x1616) is **not square** and must not be
  used for `tauri icon`; it belongs in the app as a header/about image
  (ship via `bundle.resources` when the UI needs it).
- **Generated and verified:** `32x32.png`, `128x128.png`, `128x128@2x.png`
  (256x256), `icon.ico` (6 embedded sizes, 48 KB), `icon.icns` (valid
  `icns` magic, 672 KB) — plus Windows Store tiles and Android/iOS sets,
  which the CLI throws in for free.
- The draft `tauri.conf.json` references exactly the five files the
  bundler consumes:
  `icons/32x32.png`, `icons/128x128.png`, `icons/128x128@2x.png`,
  `icons/icon.icns`, `icons/icon.ico`.

Re-running after a logo tweak is one command:
`npx tauri icon src-tauri/icons/icon-transparent.png`.

## 4. Binary size expectations

Verified against Tauri v2 docs and community reports (not guesses):

- Rust release binary: **~3–10 MB** (vs Electron's ~100–150 MB baseline).
- Windows NSIS/MSI installer: **~4–15 MB** with the WebView2 bootstrap path.
- macOS DMG: similar order, single-digit MB for a small app.
- Linux AppImage: larger if `bundleMediaFramework` pulls in codecs —
  budget **~15–30 MB** for a recorder that needs them.

The webview itself is **not** bundled (OS-provided), which is where the
size win comes from.

## 5. What's in this spike branch

```
src-tauri/
  tauri.conf.json   # minimal draft (bundle section above)
  Cargo.toml        # tauri 2, custom-protocol feature
  build.rs          # tauri_build::build()
  src/main.rs       # default builder
  icons/            # real `tauri icon` output from our transparent logo
```

Not included on purpose: frontend (`npm run dev/build` placeholders in
config), capture engine (native screen capture is a separate spike —
likely Rust `xcap`-style crates behind Tauri commands), signing/notarization,
updater keys.

## 6. Open decisions (need Taha/DEVagent)

1. **Identifier:** draft uses `com.flickcut.app` (placeholder). Must be
   final before first public build — it becomes the bundle ID, install
   paths, and the WebView data directory.
2. **Frontend stack:** config assumes Vite on port 1420 (`npm run dev`).
   Confirm React/Vue/Svelte choice.
3. **Windows target priority:** NSIS first (simplest UX). MSI only if
   enterprise deployment matters.
4. **Signing:** Windows cert + Apple Developer Program ($99/yr) needed
   before public installers; unsigned builds are fine for internal testing.
