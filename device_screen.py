# Device status: system, network and storage details, refreshed live.
import gc
import machine
import network
import os
import sys
import time
import net
from gfx import W, BG, FG, DIM, MUTED, PANEL, PANEL2, ACCENT, GREEN, YELLOW, ORANGE, RED
from ui import Screen, Button, HEADER

REFRESH_S = 2
STORAGE_REFRESH_S = 15
ROW_Y = 100
ROW_H = 30
BAR_X, BAR_W = 136, 80     # between the label and the (<=14 char) value
TABS = ('System', 'Network', 'Storage')

_adc = machine.ADC(4)    # RP2040 internal temperature sensor


def kb(n):
    return '%dK' % (n // 1024) if n < 1024 * 1024 else '%.1fM' % (n / 1048576)


def level_color(pct):
    return GREEN if pct < 60 else YELLOW if pct < 85 else RED


def cpu_temp():
    v = _adc.read_u16() * 3.3 / 65535
    return 27 - (v - 0.706) / 0.001721


def uptime():
    s = time.ticks_ms() // 1000      # ms since boot (wraps after ~12 days)
    d, s = divmod(s, 86400)
    h, s = divmod(s, 3600)
    m = s // 60
    return ('%dd %dh %dm' % (d, h, m)) if d else ('%dh %dm' % (h, m)) if h else ('%dm %ds' % (m, s % 60))


def signal(rssi):
    """-> (label, 0-100 quality)."""
    q = min(100, max(0, 2 * (rssi + 100)))
    label = 'Excellent' if rssi > -55 else 'Good' if rssi > -67 else 'Fair' if rssi > -75 else 'Weak'
    return label, q


class DeviceScreen(Screen):
    feature = 'device'
    title = 'Device'

    def __init__(self, app):
        super().__init__(app)
        self.tab = 0
        self._last = 0
        self._build()

    def back(self):
        import screens
        self.app.go(screens.SettingsScreen(self.app))

    def _build(self):
        self.buttons = self.buttons[:1]
        tw = (W - 24 - 16) // 3
        for i, name in enumerate(TABS):
            sel = i == self.tab
            self.buttons.append(Button(12 + i * (tw + 8), 54, tw, 36, name, lambda i=i: self.switch(i),
                                       ACCENT if sel else PANEL, BG if sel else DIM))

    def switch(self, i):
        self.tab = i
        self._build()
        self.enter()

    # --- rows: (label, value, bar percent or None, bar colour) ---

    def _system(self):
        gc.collect()
        free, used = gc.mem_free(), gc.mem_alloc()
        ram = used * 100 // (free + used)
        load = self.app.cpu_load()
        temp = cpu_temp()
        v = sys.implementation.version
        return (
            ('Firmware', 'MicroPython %d.%d.%d' % v[:3], None, 0),
            ('Board', 'Pico W (RP2040)', None, 0),
            ('CPU', '%dMHz %d%%' % (machine.freq() // 1000000, load), load, level_color(load)),
            ('Temp', '%.1f C' % temp, None, 0),
            ('RAM', '%s/%s %d%%' % (kb(used), kb(free + used), ram), ram, level_color(ram)),
            ('RAM free', kb(free), None, 0),
            ('Uptime', uptime(), None, 0),
        )

    def _network(self):
        w = net.wlan
        if not w.isconnected():
            return (('Status', 'Disconnected', None, 0),
                    ('SSID', net.config.WIFI_SSID or '-', None, 0))
        ip, mask, gw, dns = w.ifconfig()
        rssi = w.status('rssi')
        label, q = signal(rssi)
        mac = ':'.join('%02X' % b for b in w.config('mac'))
        return (
            ('Status', 'Connected', None, 0),
            ('SSID', w.config('ssid')[:18], None, 0),
            ('IP', ip, None, 0),
            ('Gateway', gw, None, 0),
            ('Signal', '%d dBm %s' % (rssi, label), q, GREEN if q >= 66 else YELLOW if q >= 50 else ORANGE),
            ('MAC', mac, None, 0),
            ('Hostname', network.hostname(), None, 0),
        )

    def _storage(self):
        bsize, _, blocks, bfree = os.statvfs('/')[:4]
        total, free = bsize * blocks, bsize * bfree
        used = total - free
        pct = used * 100 // total
        code = data = n = 0
        for entry in os.ilistdir('/'):
            name = entry[0]
            if entry[1] != 0x8000:     # regular files only
                continue
            n += 1
            size = os.stat(name)[6]
            if name.endswith('.py') or name.endswith('.mpy'):
                code += size
            else:
                data += size
        return (
            ('Flash', kb(total), None, 0),
            ('Used', '%s  %d%%' % (kb(used), pct), pct, level_color(pct)),
            ('Free', kb(free), None, 0),
            ('Files', '%d' % n, None, 0),
            ('App code', kb(code), None, 0),
            ('Data', kb(data), None, 0),
        )

    def draw(self):
        super().draw()
        self._draw_rows()

    def _draw_rows(self):
        cv = self.cv
        cv.fill_rect(0, ROW_Y - 4, W, H_ROWS, BG)
        try:
            rows = (self._system, self._network, self._storage)[self.tab]()
        except Exception as e:
            rows = (('Error', str(e)[:24], None, 0),)
        for i, (label, value, bar, color) in enumerate(rows):
            y = ROW_Y + i * ROW_H
            if i % 2 == 0:
                cv.fill_rect(12, y - 4, W - 24, ROW_H, PANEL)
            cv.big_text(label, 24, y + 3, DIM, 2)
            if bar is not None:
                cv.fill_rect(BAR_X, y + 6, BAR_W, 10, PANEL2)
                cv.fill_rect(BAR_X, y + 6, BAR_W * min(bar, 100) // 100, 10, color)
            cv.rtext(value, W - 24, y + 3, FG, 2)
        self._last = time.time()

    def second(self, t):
        # Storage barely changes and walking the filesystem isn't free
        every = STORAGE_REFRESH_S if self.tab == 2 else REFRESH_S
        if time.time() - self._last >= every:
            self._draw_rows()
            self.cv.flush(0, ROW_Y - 4, W, H_ROWS)


H_ROWS = 320 - (ROW_Y - 4)
