# pico-thing

A Raspberry Pi Pico W with a Waveshare Pico-ResTouch-LCD-3.5 touchscreen, running
MicroPython as a little computer on my desk.

It shows clock and weather faces and a Pomodoro timer. It also controls my Yeelight
lamps and Sonos speakers over the local network. A small server on my Mac
(`companion/`) tells it what Claude Code is doing.

## Setup

1. `cp config.example.py config.py` and fill in your Wi-Fi and device details.
2. `firmware/build.sh && firmware/flash.sh` builds MicroPython with the app frozen in
   (this needs Docker) and flashes the Pico.
3. On the Mac, run `python3 companion/server.py`, or install it as a LaunchAgent and
   manage it with `companion/pico-thing`.

Ideas and unfinished work are in `TODO.md`.
