#!/bin/sh
# Reboot the Pico into its USB bootloader (the RPI-RP2 drive) by sending "b" over its
# USB serial port. The firmware waits for the port to close before rebooting: a
# device vanishing while macOS holds the port open can panic the Mac.
[ -d /Volumes/RPI-RP2 ] && exit 0
for port in /dev/cu.usbmodem*; do
    [ -e "$port" ] && printf b > "$port" 2>/dev/null
done
for _ in $(seq 1 40); do [ -d /Volumes/RPI-RP2 ] && exit 0; sleep 0.5; done
echo "Pico bootloader drive didn't appear (hold BOOTSEL while plugging it in)" >&2
exit 1
