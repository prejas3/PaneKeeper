CCS/1
P:PaneKeeper (folder and Hive project are still named WindowsKeeper)
PURPOSE:save and restore Windows app layouts across monitors
DEP:Tauri2,Rust,vanilla TS+Vite,windows-sys
RULE:minimal dependencies; targeted tests plus one full run per card
RULE:comments,notes,summaries=CCS
RULE:UI=custom controls; no default browser/OS controls
RULE:per-monitor-v2 DPI aware
!SEC:user-mode only; no elevation,hooks,injection
!SEC:move/resize only; never close another app window
STATE:%APPDATA%/PaneKeeper
DEC:dev server port 1431
RULE:no push/release by agents; Szymon publishes
CHECK:npm run e2e = real release exe over WebView2 DevTools; moves only testwin.exe windows (src-tauri/examples/testwin.rs)
CHECK:live tests = cargo test -- --include-ignored (parallel-safe, testwin windows only)
