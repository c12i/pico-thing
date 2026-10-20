# User preferences persisted to flash as JSON.
import json

FILE = 'settings.json'
DEFAULTS = {'face': 'digital', 'calib': None, 'lights': [], 'theme': 'dark', 'brightness': 100}


def load():
    s = dict(DEFAULTS)
    try:
        with open(FILE) as f:
            s.update(json.load(f))
    except (OSError, ValueError):
        pass
    return s


def save(s):
    with open(FILE, 'w') as f:
        json.dump(s, f)
