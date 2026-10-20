#!/bin/sh
# Quick deploy for iterating. Most modules are frozen into the firmware (see
# firmware/); this only copies modules changed since the last firmware build, as
# .mpy files that shadow the frozen copies. Those cost RAM, so once things work run:
#   firmware/build.sh && firmware/flash.sh
set -e
cd "$(dirname "$0")"
MPREMOTE=$(command -v mpremote || echo "$HOME/.local/bin/mpremote")
MPYCROSS=$(command -v mpy-cross || echo "$HOME/.local/bin/mpy-cross")
UF2=firmware/pico-thing.uf2

rm -rf build && mkdir build
cp main.py config.py build/
STALE=""
for f in *.py; do
  case "$f" in main.py|config.py|config.example.py) continue ;; esac
  if [ ! -f "$UF2" ] || [ "$f" -nt "$UF2" ]; then
    "$MPYCROSS" -march=armv6m -o "build/${f%.py}.mpy" "$f"
    echo "shadowing frozen $f"
  fi
  STALE="$STALE\"$f\","
done

# A .py on the device would shadow its .mpy, so remove those first
"$MPREMOTE" exec "import os
for f in ($STALE):
    try:
        os.remove(f)
    except OSError:
        pass"
cd build && "$MPREMOTE" cp * : + reset
