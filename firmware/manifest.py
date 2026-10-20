# Frozen into the firmware: bytecode and strings live in flash, not RAM.
# Every .py under pico/<folder>/ is frozen as a top-level module (the folders only
# organise the repo). pico/main.py and pico/config.py stay on the filesystem so they
# can be edited without a rebuild.
include("$(BOARD_DIR)/manifest.py")

for folder in ("core", "devices", "drivers", "faces", "net", "screens"):
    freeze("/app/pico/" + folder)
