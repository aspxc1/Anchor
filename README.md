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

## Focus enforcement acceptance check

1. Start a session with only Notepad selected. Switch to a different app on each connected display. Confirm the curtain covers the display that contains the blocked app and names it.
2. Click **Return to Focus** with Notepad first visible, then minimized. Confirm Notepad returns to the foreground, the curtain hides, and Notepad's native **Save As** dialog is allowed.
3. Open Task Manager during enforcement and confirm it stays usable. Close every allowed window, then switch to another application. Choose a replacement from the curtain and confirm the session continues.
4. Start an emergency hold, release it early, then verify a new hold starts from zero. Try the typed challenge and the full ten-second hold in separate sessions. Both should open the dashboard and leave focus enforcement stopped.

## Timer and Hall Pass acceptance check

1. Start a custom 1-minute focus and 1-minute break with two passes. Compare phases and timers between the HUD and the dashboard opened from the tray.
2. Activate a Hall Pass from the curtain and switch apps. Leave a blocked app in the foreground until the pass reaches zero. Confirm the curtain returns immediately and the focus clock resumes where it paused. Use the second pass from the HUD and end it early.
3. During a break, switch freely and add a new window from the dashboard. Start the next focus sprint early. Verify the added window is now allowed, then end the session during the next break.
4. Drag the HUD left and right. Confirm it stays at the top of the primary work area. Confirm **Quit** from the tray removes the HUD and ends enforcement.
