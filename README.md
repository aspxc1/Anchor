# Anchor

Anchor is a Tauri desktop app for Windows 10 and Windows 11 x64. The current milestone lets you discover, search, refresh, and select open windows for a future focus session.

## Development

Install Node.js 24, the stable Rust toolchain, Visual Studio C++ Build Tools with the Windows SDK, and Microsoft Edge WebView2 Runtime. Then run:

```powershell
npm ci
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

## Window discovery acceptance check

1. Open Notepad and a second desktop app. Open Calculator and Settings, then close them. Launch Anchor from the command above.
2. Confirm the open apps appear with their window titles and executable names. Confirm Anchor, closed apps, and cloaked background windows are absent.
3. Select multiple windows, temporarily hide them from the list with a search, and clear the search. The selections should stay in the allowlist.
4. Close a selected window and click **Refresh**. Verify that only the closed window is removed from the allowlist. Open a new window and verify that it appears after refreshing.
5. Minimize a visible, titled app and refresh. It should remain available for selection when Windows still reports it as visible and uncloaked.
