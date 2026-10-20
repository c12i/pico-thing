# pico-thing

A Raspberry Pi Pico W with a Waveshare Pico-ResTouch-LCD-3.5 touchscreen, running
MicroPython as a little computer on my desk.

It shows clock and weather faces and a Pomodoro timer. It also controls my Yeelight
lamps and Sonos speakers over the local network. A small server on my Mac
(`companion/`) tells it what Claude Code is doing.

## Layout

```
pico/            code that runs on the Pico
  main.py        entry point
  config.py      your settings and secrets (gitignored; see config.example.py)
  core/          app loop, drawing, icons, settings, Pomodoro state
  drivers/       LCD and touch panel
  net/           Wi-Fi, HTTP, weather, Mac companion client
  devices/       Yeelight, Xiaomi miIO and Sonos clients
  faces/         home screen faces, one file each
  screens/       menu, settings and feature screens
companion/       Mac server, Claude Code hook and the pico-thing CLI
firmware/        build, flash and deploy scripts
```

The folders only organise the repo. On the Pico every module is top level, so code
says `import lcd`, not `import drivers.lcd`.

## Setup

1. `cp pico/config.example.py pico/config.py` and fill in your Wi-Fi and device details.
2. `firmware/build.sh && firmware/flash.sh` builds MicroPython with the app frozen in
   (this needs Docker) and flashes the Pico. `firmware/deploy.sh` is quicker while
   iterating: it copies only the changed modules.
3. On the Mac, run `python3 companion/server.py`, or install it as a LaunchAgent and
   manage it with `companion/pico-thing`.

Ideas and unfinished work are in `TODO.md`.
