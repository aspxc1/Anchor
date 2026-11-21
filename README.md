# Anchor

Anchor is a desktop focus app for Windows 10 and Windows 11 (x64) built with Tauri. Pick the exact windows and websites you need, set a goal, and work in Pomodoro sprints while keeping distractions out of the way. All notes, profiles, and session history stay local.

## Features

- **Window-level focus enforcement**: Allow only the windows you select (and their native dialogs). Switching to any other app brings up a focus curtain across single or multi-monitor setups, with temporary **Hall Passes** and **Emergency Unlock** when needed.
- **Pomodoro timer & floating HUD**: Configure focus and break intervals with a compact, always-on-top HUD and system tray controls.
- **Study profiles**: Save window selections, allowed websites, timer lengths, and audio preferences for one-click launch (with optional auto-launch for closed apps).
- **Browser allowlisting**: Pair the bundled Chromium extension (Chrome, Edge, Brave) to allow specific domains, wildcard subdomains (e.g. `*.mit.edu`), and path prefixes (e.g. `github.com/myorg/*`) during focus sprints.
- **Brain Dump & ambient noise**: Capture quick thoughts without breaking focus using `Ctrl+Shift+Space` (customizable in **Settings**), and optionally play built-in brown or pink noise during sprints.
- **Local history & analytics**: Review completed sessions, streaks, blocked apps/websites, and 12-week focus activity in **History**.

## Usage

1. Launch Anchor and select the open windows you want to allow (or click a saved **Study profile**).
2. Optionally set a goal, allowed websites, focus/break durations, Hall Passes, and ambient noise.
3. Start the session. During focus sprints, Anchor keeps you in your allowed windows; during breaks, you can switch freely or adjust your allowlist.
4. Open **Settings** to customize the Brain Dump shortcut, toggle **Start Anchor with Windows**, or pair the browser extension.

### Browser Extension (Optional)

In **Settings → Browser bridge**, click **Open extension folder** to locate the unpacked extension (`extensions/chromium`), load it in Chrome, Edge, or Brave, and pair it using the local port and pairing token. See [extensions/chromium/README.md](extensions/chromium/README.md) for step-by-step instructions.

### Local Data

Anchor stores everything locally under `%APPDATA%\Anchor`:

- `anchor.db` — sessions, profiles, preferences, and Brain Dump notes (SQLite)
- `braindump\` — daily Markdown exports of your Brain Dump notes

## Installation

On Windows 10/11 (x64), run the installation script in PowerShell to check/install prerequisites, build Anchor, and launch the installer:

```powershell
irm https://raw.githubusercontent.com/aspxc1/Anchor/main/install.ps1 | iex
```

Or from a local clone:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

## Building from Source

Requires Windows 10/11 (x64) with Node.js 24, the stable Rust toolchain, Visual Studio C++ Build Tools (with the Windows SDK), and the Microsoft Edge WebView2 Runtime.

```powershell
npm ci
npm run tauri dev    # Run in development mode
npm run tauri build  # Build NSIS and MSI installers
```
