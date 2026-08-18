# Media Server Integration & Metadata Standards

`dvd-ripper` provides native, zero-configuration integration with popular home media server platforms (**Plex**, **Jellyfin**, **Emby**, and **Kodi**).

---

## 1. Automatic Library Refresh Triggers

When configured, `dvd-ripper` automatically issues HTTP library scan requests to your media server immediately after a DVD backup job completes.

### 1.1 Plex Media Server Integration
Configure your Plex base URL and X-Plex-Token:

- **Via CLI Flags**:
  ```bash
  dvd-ripper --input D:\ --search "Aliens" --plex-url "http://192.168.1.100:32400" --plex-token "YOUR_PLEX_TOKEN"
  ```
- **Via Configuration File (`dvd-ripper.toml`)**:
  ```toml
  plex_url = "http://192.168.1.100:32400"
  plex_token = "YOUR_PLEX_TOKEN"
  ```

Upon rip completion, `dvd-ripper` triggers:
```http
GET http://192.168.1.100:32400/library/sections/all/refresh?X-Plex-Token=YOUR_PLEX_TOKEN
```

---

### 1.2 Jellyfin Media Server Integration
Configure your Jellyfin base URL and API Key:

- **Via Configuration File (`dvd-ripper.toml`)**:
  ```toml
  jellyfin_url = "http://192.168.1.100:8096"
  jellyfin_key = "YOUR_JELLYFIN_API_KEY"
  ```

Upon rip completion, `dvd-ripper` triggers:
```http
POST http://192.168.1.100:8096/Items/Root/Refresh?api_key=YOUR_JELLYFIN_API_KEY
```

---

### 1.3 Emby Media Server Integration
Configure your Emby base URL and API Key:

- **Via Configuration File (`dvd-ripper.toml`)**:
  ```toml
  emby_url = "http://192.168.1.100:8096"
  emby_key = "YOUR_EMBY_API_KEY"
  ```

Upon rip completion, `dvd-ripper` triggers:
```http
POST http://192.168.1.100:8096/Library/Refresh?api_key=YOUR_EMBY_API_KEY
```

---

## 2. NFO Metadata Sidecar File Generation (`.nfo`)

When `--nfo` or `nfo = true` is enabled, `dvd-ripper` creates an XML NFO metadata sidecar file alongside the output video file (e.g. `Films/Aliens (1986)/Aliens (1986).nfo`).

### NFO Schema Example
```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<movie>
  <title>Aliens</title>
  <originaltitle>Aliens</originaltitle>
  <year>1986</year>
  <plot>Fifty-seven years after surviving the apocalyptic attack on Nostromo, Ellen Ripley is rescued by a deep-space recovery team...</plot>
  <rating>8.4</rating>
  <director>James Cameron</director>
</movie>
```

---

## 3. Folder & Cover Artwork Standards

When online metadata resolution retrieves poster artwork from OMDb/TMDB, `dvd-ripper` saves local image files alongside the media container:

- `cover.jpg`: Standard poster artwork file.
- `folder.jpg`: Directory thumbnail thumbnail used by Windows Explorer and network SMB shares.
