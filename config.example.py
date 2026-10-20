# Local settings for pico-thing. Copy to config.py and fill in; config.py is gitignored.
WIFI_SSID = "your-network"
WIFI_PASSWORD = "your-password"

# Leave as None to auto-detect from your public IP
LATITUDE = None
LONGITUDE = None
CITY = None

UNITS = "C"   # "C" or "F"

# Optional friendly names for Yeelights, keyed by IP (or Yeelight id)
LIGHT_NAMES = {
    # "192.168.1.13": "Corner lamp",
}

# Sonos speakers as (name, IP). Reserve these IPs in your router so they don't move.
SONOS = (
    # ("Living room", "192.168.1.5"),
)

# Xiaomi-ecosystem lights (no Yeelight LAN Control) controlled over miIO.
# IP -> 32-hex-char device token from your Xiaomi account.
MIIO_TOKENS = {
    # "192.168.1.14": "0123456789abcdef0123456789abcdef",
}

# Mac companion (companion/server.py). The token must match companion/config.json,
# which the server creates on its first run.
COMPANION = "http://192.168.1.8:8765"
COMPANION_TOKEN = ""

# Optional quotes for the Productivity face (built-in ones are used if unset).
# Up to ~50 characters fits on two lines; plain ASCII only.
# QUOTES = (
#     'Lock in.',
# )
