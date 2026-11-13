# Universal Embedded Web Dashboard & Remote Controller - `dvd-ripper`

`dvd-ripper` embeds a lightweight, zero-dependency HTML5/CSS3/JavaScript responsive web dashboard (`src/api.rs`), enabling complete remote monitoring, status telemetry, acoustic mode switching, candidate selection, and job cancellation from any modern web browser or mobile smartphone on the local network.

---

## 1. Accessing the Dashboard

When running `dvd-ripper` in daemon mode (`--daemon`):

```bash
dvd-ripper --daemon --drive-pool "D:\,E:\"
```

Open your browser to:
- **`http://localhost:8080/`** or **`http://localhost:8080/dashboard`**
- From another device on your local LAN: **`http://<SERVER_IP>:8080/dashboard`**

---

## 2. Dashboard Interface & Capabilities

The dashboard is structured into four main operational views:

```text
┌────────────────────────────────────────────────────────────────────────┐
│ 📀 DVD Ripper Embedded Appliance                             [ONLINE]  │
├────────────────────────────────────────────────────────────────────────┤
│ Optical Drive Pool Status                                [🔄 Refresh]  │
│ 💿 Optical Drive D:                                      [ Ripping ]   │
│ Drive: D: | Disc: THE_MATRIX | Title: The Matrix | FPS: 45 | Speed: 2.4x│
│ Progress: [====================>              ] 64.2%                  │
│ [▶ Start Rip]  [⏹ Cancel]  [⏏ Eject Tray]                             │
│ Acoustic Profile: [🔇 Silent (4x)] [⚖️ Balanced (8x)] [🚀 Turbo (16x)] │
├────────────────────────────────────────────────────────────────────────┤
│ 🔍 IMDb Metadata Search & Candidate Selection                          │
│ [ Search title... (e.g. Kill Bill, Aliens)             ] [🔍 Search]   │
│ - Kill Bill: Vol. 1 (2003) [tt0266697]                     [Select]    │
│ - Kill Bill: Vol. 2 (2004) [tt0378194]                     [Select]    │
├────────────────────────────────────────────────────────────────────────┤
│ Ripping History                                                        │
│ - The Matrix (Movie) - Films/The Matrix (1999)/The Matrix.mp4 [Success]│
└────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Live Drive Pool Monitoring
- **Multi-Drive Awareness**: Displays independent cards for every optical drive discovered or configured via `--drive-pool`.
- **Real-Time Progress**: Smooth, animated progress bars updated every 500ms via Server-Sent Events (SSE).
- **Encoding Telemetry**: Real-time display of current frame rate (FPS), ripping speed multiplier (`2.4x`), and active disc label.

### 2.2 Live Remote Control Buttons
- **`▶ Start Rip`**: Initiates background extraction once a disc is inserted and the title is verified.
- **`⏹ Cancel`**: Instantly signals cooperative cancellation to the active FFmpeg worker for that drive.
- **`⏏ Eject Tray`**: Opens the optical tray.
- **Acoustic Profile Buttons**:
  - `🔇 Silent (4x)`: Instantly commands drive firmware to throttle spindle speed for quiet operation.
  - `⚖️ Balanced (8x)`: Restores standard operating speed.
  - `🚀 Turbo (16x)`: Maximizes drive hardware capability.

### 2.3 Interactive Title Search & Verification
When an ambiguous volume label (e.g. `DVD_VIDEO`) is detected:
1. Search the target movie or TV show using the search bar.
2. Review matching candidates (title, release year, IMDb ID, movie poster).
3. Click **Select for <Drive>** to bind the metadata and unlock the **▶ Start Rip** button.

### 2.4 Live History Feed
Inspect past completed, cancelled, or failed rip jobs directly in the browser with output file paths, timestamps, and status badges.

---

## 3. Real-Time Telemetry: Server-Sent Events (SSE)

The dashboard establishes a persistent Server-Sent Events (`text/event-stream`) connection at `GET /api/events`:

```javascript
const evtSource = new EventSource('/api/events');
evtSource.onmessage = function(e) {
    const data = JSON.parse(e.data);
    renderStatusData(data);
};
```

This eliminates aggressive HTTP polling loops while providing responsive progress updates with minimal server CPU overhead (<0.1% CPU).

---

## 4. Mobile & Touch Screen Optimization

The dashboard is engineered with mobile-first CSS:
- Responsive CSS Grid and Flexbox layouts adapted for smartphones, tablets, and desktop displays.
- Large, touch-friendly buttons (`min-height: 44px`) adhering to mobile accessibility standards.
- Dark mode aesthetic optimized for low-light homelab server closets and living rooms.
- Zero external CDNs or JavaScript frameworks (no React, Vue, or Tailwind downloads required); the page loads instantaneously even when the server is disconnected from the public internet.
