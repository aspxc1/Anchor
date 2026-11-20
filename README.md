# Anchor

Anchor is a Tauri desktop focus app for Windows 10 and Windows 11 x64. Choose the exact windows you need, set a goal, and work in Pomodoro sprints. Anchor allows selected windows and their dialogs, and keeps Brain Dumps and session history local.

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
4. Drag the HUD to a second monitor. Confirm it snaps to the top of that work area, then restart Anchor and verify that the placement survives. Confirm **Quit** from the tray removes the HUD and ends enforcement.

## Brain Dump and audio acceptance check

1. Start a focus session with noise enabled. Try brown and pink noise and the volume slider. Confirm toggling mute from the HUD fades the sound and entering a break pauses it.
2. During focus, press **Ctrl+Shift+Space**, type a note, and press **Enter**. Confirm the popup closes and returns you to an allowed window. Verify the note in `%APPDATA%\Anchor\anchor.db` and in today's file in `%APPDATA%\Anchor\braindump`.
3. Repeat with **Esc** and by clicking away. Confirm neither dismissal saves a note. Change the shortcut in **Settings**, retry capture, and restart Anchor to verify that the shortcut is retained.

Anchor's daily Markdown files are generated from SQLite. An export that fails is queued for retry on the next capture or launch.

## Session history acceptance check

1. Block several switches to the same app, capture a note, then end the session during a break. Confirm the dashboard opens the session summary with actual focus time, Hall Pass usage, grouped blocked switches, and the captured note.
2. Capture an unlinked note with no running session. In **Brain Dumps**, search for that note, copy it, open the Markdown folder, then delete it. Confirm it is removed from both the viewer and the daily Markdown file.
3. Restart Anchor and reopen **History**. Confirm the completed session and streak persist. Repeat a session with Emergency Unlock and confirm its history status is **Aborted**.

On Windows x64, build the installers with `npm run tauri build`. Local cross-target Rust checks and portable tests do not exercise desktop focus behavior, WebView2 audio, or installer startup.


## Mixed-DPI Windows acceptance check

- Place monitors at different scaling factors, with one to the left of the primary screen. Trigger the curtain on both monitors and confirm there are no gaps or overflow.
- Return to a maximized VS Code or browser window and confirm it stays maximized. Minimize it and repeat. Close the most recently allowed window to test fallback to a remaining one.
- Confirm the Windows Store or another hosted UWP app displays its own executable instead of ApplicationFrameHost. Open Alt+Tab and taskbar previews, then switch to a blocked window to confirm enforcement still activates.
