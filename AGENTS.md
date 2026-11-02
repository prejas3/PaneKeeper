CCS/1
P:WindowsKeeper
PURPOSE:save and restore Windows app layouts across monitors
DEP:Tauri2,Rust,vanilla TS+Vite,windows-sys
RULE:minimal dependencies; targeted tests plus one full run per card
RULE:comments,notes,summaries=CCS
RULE:UI=custom controls; no default browser/OS controls
RULE:per-monitor-v2 DPI aware
!SEC:user-mode only; no elevation,hooks,injection
!SEC:move/resize only; never close another app window
STATE:%APPDATA%/WindowsKeeper
DEC:dev server port 1431
RULE:no push/release by agents; Szymon publishes
