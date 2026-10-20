# Frozen into the firmware: bytecode and strings live in flash, not RAM.
# main.py and config.py stay on the filesystem so they can be edited without a rebuild.
include("$(BOARD_DIR)/manifest.py")

freeze("/app", (
    "ahttp.py",
    "bonsai.py",
    "bot.py",
    "command_screen.py",
    "companion.py",
    "device_screen.py",
    "faces.py",
    "gfx.py",
    "icons.py",
    "lcd.py",
    "lights_screen.py",
    "miio.py",
    "net.py",
    "pomodoro.py",
    "pomodoro_screen.py",
    "screens.py",
    "settings.py",
    "sonos.py",
    "sonos_screen.py",
    "touch.py",
    "ui.py",
    "weather.py",
    "yeelight.py",
))
