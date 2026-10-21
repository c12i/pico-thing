# TODO

- [ ] Bedside lamp token (miIO)
- [ ] Approve prompts from Pico
- [ ] Quick panel on home face
- [ ] Auto-rediscover device IPs
- [ ] Photo wallpaper face
- [ ] Custom tasks + Shortcuts
- [ ] Sonos recently played (low priority)

## Bedside lamp (bslamp2) via miIO
The bedside lamp (`yeelink.light.bslamp2`, 192.168.1.14) has no Yeelight LAN Control,
so the Pico talks to it over Xiaomi's miIO protocol (`core/src/net/miio.rs`, written but
untested on the lamp). It just needs the device token.

1. Extract the token (log in with the Xiaomi account the lamp is registered to;
   leave the server region blank to try all):
   ```sh
   git clone https://github.com/PiotrMachowski/Xiaomi-cloud-tokens-extractor ~/tools/xiaomi-tokens
   cd ~/tools/xiaomi-tokens
   python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
   .venv/bin/python token_extractor.py
   ```
2. Copy the TOKEN for model `yeelink.light.bslamp2` / IP 192.168.1.14 into `config.toml`:
   ```toml
   [miio_tokens]
   "192.168.1.14" = "<32-char token>"
   ```
3. `./flash.sh`, then check Menu → Lights → Bedside lamp.

## Approve Claude Code permission prompts from the Pico
A `PermissionRequest` hook can return an allow/deny decision. Plan: the hook waits up
to ~25s for an Allow / Deny / "Use terminal" tap on the Pico; no answer falls back to
the normal terminal prompt. The terminal prompt is held back while it waits, so only
wait when the Pico has polled the companion recently. Show the full command/file on
the Pico before allowing.

## Quick panel on the home face
Tap the bottom of a face to open light toggles + now playing (play/pause, skip,
volume) instead of going through the menu. Tapping elsewhere still opens the menu.

## Auto-rediscover device IPs
- Sonos: remember each speaker's UUID; when one is unreachable, SSDP-search and
  update its IP.
- Yeelight: re-scan automatically when a cached light goes offline, match by id.
- Mac companion: if unreachable, the Pico broadcasts a token-signed hello on UDP and
  the companion answers, so a new Mac IP is picked up without editing config.toml.

## Photo wallpaper face
`companion/wallpaper.py photo.jpg` crops and resizes a photo to 480x320 with `sips`,
dithers it to 16 colours and saves `wall.bin` (palette plus 4-bit pixels, 77KB). A
"Photo" face fetches it from the companion and streams it to the panel in strips
(there's no room for a framebuffer), then draws the clock on top.

## Custom companion tasks + Shortcuts
Candidates: "Open PRs" (`gh pr list` across repos), "Standup draft" (yesterday's
commits across ~/coding), test runs per project. Shortcuts already on the Mac:
Night Light, Take a Break, Morning Routine, Going Home.

## Sonos recently played (low priority)
Sonos keeps no play history on the speakers (the app's "Recently Played" comes from
Sonos's cloud and needs an account login). To get one, the companion would watch the
speakers (GetMediaInfo / GetPositionInfo every ~30s), keep the last ~30 distinct
playlists, stations and tracks with their URI and metadata in a file on the Mac, and
replay one on request the way `play_favorite` does (SetAVTransportURI for streams,
the queue for everything else).
