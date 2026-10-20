#!/bin/sh
# Build MicroPython for the Pico W with the app frozen in (needs Docker/OrbStack).
# Output: firmware/pico-thing.uf2
set -e
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
IMAGE=pico-thing-fw

[ -d micropython ] || git clone --depth 1 --branch v1.29.0 https://github.com/micropython/micropython.git micropython

docker build -q -t "$IMAGE" . >/dev/null
docker run --rm \
    -v "$ROOT/firmware/micropython:/work/micropython" \
    -v "$ROOT:/app:ro" \
    "$IMAGE" sh -ec '
        cd /work/micropython
        [ -x mpy-cross/build/mpy-cross ] || make -C mpy-cross -j"$(nproc)" >/dev/null
        [ -f lib/pico-sdk/src/rp2_common/CMakeLists.txt ] || make -C ports/rp2 BOARD=RPI_PICO_W submodules
        make -C ports/rp2 BOARD=RPI_PICO_W FROZEN_MANIFEST=/app/firmware/manifest.py -j"$(nproc)" >/dev/null
    '
cp micropython/ports/rp2/build-RPI_PICO_W/firmware.uf2 pico-thing.uf2
ls -l pico-thing.uf2
