# WindowsKeeper

WindowsKeeper is a lightweight Windows app that saves and restores complete multi-monitor window layouts.

## Development

Requirements: Node.js, Rust, the MSVC build tools, and the WebView2 runtime.

```powershell
npm install
npm run tauri dev
```

Run the engine tests from `src-tauri` with `cargo test`. The ignored `live_notepad_round_trip_within_two_pixels` test opens two uniquely named Notepad windows, moves only those windows, restores them, and checks their frame rectangles.

## Safety

WindowsKeeper runs without elevation. It does not inject code, install hooks, or close other applications; it only reads and updates top-level window placement.

Licensed under the MIT License.
