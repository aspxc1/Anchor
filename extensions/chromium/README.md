# Anchor Focus for Chromium

This folder loads directly in Chrome, Edge, and Brave. It has no build step.

## Load and pair

1. Open `chrome://extensions`, `edge://extensions`, or `brave://extensions`.
2. Enable **Developer mode**. Click **Load unpacked** and select the `extensions/chromium` folder containing `manifest.json`.
3. In Anchor desktop, open **Session setup**, then **Settings**. Note the **Local port** and click **Copy Pairing Token**.
4. Open **Anchor Focus** from the browser’s extensions menu. Under **Settings**, enter the port, paste the token, and click **Save connection**. The badge should read **Connected**.
5. Start a session allowing your browser window and add `docs.rs` under **Allowed websites**. Navigate to `reddit.com` and confirm the extension redirects to the blocked page. Use a Hall Pass, then wait for enforcement to resume on the active tab when the pass expires.

Exact hosts, wildcard subdomains such as `*.mit.edu`, and path prefixes such as `github.com/myorg/*` are supported. Internal browser pages stay available. An empty website list or an offline Anchor bridge allows normal browsing.

Changing the desktop port or regenerating the token requires updating the extension settings. Developer mode is only needed for the unpacked installation.
