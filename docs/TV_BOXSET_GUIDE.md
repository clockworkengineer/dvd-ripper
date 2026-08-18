# TV Series Multi-Disc Box Set Auto-Stitching User Guide

`dvd-ripper` provides a **Multi-Disc TV Series Box Set Auto-Stitching Engine** (`--auto-boxset`) designed to simplify digitizing full TV show season box sets spanning 2, 3, 4, or more physical DVD discs.

---

## 1. Overview & Problem Solved

When ripping TV series season box sets:
- **Disc 1** typically contains Episodes 1 through 4 (`S01E01`, `S01E02`, `S01E03`, `S01E04`).
- **Disc 2** contains Episodes 5 through 8 (`S01E05`, `S01E06`, `S01E07`, `S01E08`).
- **Disc 3** contains Episodes 9 through 12 (`S01E09`, `S01E10`, `S01E11`, `S01E12`).

Without Auto BoxSet mode, users must manually calculate and specify `--start-episode 5` when inserting Disc 2, `--start-episode 9` when inserting Disc 3, etc.

With **Auto BoxSet Mode (`--auto-boxset`)**, `dvd-ripper` tracks cumulative episode progress per show/season in a local persistent database (`~/.dvd-ripper/boxsets.json`). On subsequent disc insertions, starting episode numbers are automatically calculated and incremented seamlessly.

---

## 2. Enabling Auto BoxSet Mode

### Via Command-Line Interface (CLI)
Add `--auto-boxset` to your TV series ripping command:

```bash
dvd-ripper --input D:\ --tv --season 1 --auto-boxset --all-episodes --search "The Office"
```

### Via Desktop Graphical User Interface (GUI)
1. Select **📺 TV Series** mode in Section 2 (Media Metadata & Mode Settings).
2. Enter the show title (e.g. `The Office`) and Season Number (`1`).
3. Check the **📦 Auto BoxSet** checkbox.
4. Click **🔍 Scan Disc** or **▶ Batch Rip All Episodes**.

### Via Configuration File (`dvd-ripper.toml` or `~/.dvd-ripper/config.toml`)
Enable box set tracking permanently across all runs:

```toml
[settings]
auto_boxset = true
out_dir = "TV"
```

---

## 3. Workflow Walkthrough

### Disc 1 Insertion
1. Insert Disc 1 of *The Office (Season 1)* into drive `D:\`.
2. Run:
   ```bash
   dvd-ripper --input D:\ --tv --season 1 --auto-boxset --all-episodes --search "The Office"
   ```
3. `dvd-ripper` scans Disc 1, finds 4 episode titles, and rips them to:
   - `TV/The Office (2005)/Season 01/The Office - S01E01.mp4`
   - `TV/The Office (2005)/Season 01/The Office - S01E02.mp4`
   - `TV/The Office (2005)/Season 01/The Office - S01E03.mp4`
   - `TV/The Office (2005)/Season 01/The Office - S01E04.mp4`
4. State recorded: `last_episode = 4`, `total_discs_ripped = 1`.

### Disc 2 Insertion
1. Eject Disc 1 and insert Disc 2 into drive `D:\`.
2. Run the exact same command:
   ```bash
   dvd-ripper --input D:\ --tv --season 1 --auto-boxset --all-episodes --search "The Office"
   ```
3. `dvd-ripper` detects previous state (`last_episode = 4`), automatically sets `--start-episode 5`, and rips the 4 titles to:
   - `TV/The Office (2005)/Season 01/The Office - S01E05.mp4`
   - `TV/The Office (2005)/Season 01/The Office - S01E06.mp4`
   - `TV/The Office (2005)/Season 01/The Office - S01E07.mp4`
   - `TV/The Office (2005)/Season 01/The Office - S01E08.mp4`
4. State updated: `last_episode = 8`, `total_discs_ripped = 2`.

---

## 4. Resetting Box Set Tracking

If you re-insert Disc 1 or start a fresh rip of a show season, reset the box set counter:

### Via REST API
```bash
curl -X POST "http://localhost:8080/api/boxset/reset?show=The%20Office&season=1"
```

### Via File Deletion
Or manually delete/edit `~/.dvd-ripper/boxsets.json`:

```json
[
  {
    "show_name": "The Office",
    "season": 1,
    "last_episode": 8,
    "total_discs_ripped": 2,
    "updated_at": "2026-08-18 15:45:10"
  }
]
```
