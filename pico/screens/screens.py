# Menu, face picker, settings and touch calibration.
import gc
import net
import settings
import gfx
import icons
from gfx import W, H, BG, FG, DIM, PANEL, PANEL2, ACCENT, ACCENT2, YELLOW, GREEN, PURPLE, RED
from ui import Screen, Button, HEADER


class Tile(Button):
    def __init__(self, x, y, w, h, label, action, accent, icon):
        super().__init__(x, y, w, h, label, action)
        self.accent = accent
        self.icon = icon

    def draw(self, cv, pressed=False):
        bg = PANEL2 if pressed else PANEL
        cv.round_rect(self.x, self.y, self.w, self.h, 14, bg)
        icons.ui_icon(cv, self.icon, self.x + self.w // 2, self.y + 46, self.accent, bg)
        cv.ctext(self.label, self.x + self.w // 2, self.y + self.h - 34, FG, 2)


class Launcher(Screen):
    title = 'Menu'

    def __init__(self, app):
        super().__init__(app)
        items = (
            ('Faces', ACCENT, 'faces', lambda: app.go(FacePicker(app))),
            ('Lights', YELLOW, 'lights', self.lights),
            ('Sonos', GREEN, 'sonos', self.sonos),
            ('Command', PURPLE, 'command', self.command),
            ('Settings', DIM, 'settings', lambda: app.go(SettingsScreen(app))),
            ('Pomodoro', RED, 'pomodoro', self.pomodoro),
        )
        tw, th, gap = 148, 122, 8
        x0, y0 = (W - 3 * tw - 2 * gap) // 2, HEADER + 10
        for i, (label, color, icon, action) in enumerate(items):
            self.buttons.append(Tile(x0 + (i % 3) * (tw + gap), y0 + (i // 3) * (th + gap),
                                     tw, th, label, action, color, icon))

    def back(self):
        self.app.home()

    def pomodoro(self):
        import pomodoro_screen
        self.app.go(pomodoro_screen.PomodoroScreen(self.app))

    def lights(self):
        import lights_screen
        self.app.go(lights_screen.LightsScreen(self.app))

    def command(self):
        import command_screen
        self.app.go(command_screen.CommandScreen(self.app))

    def sonos(self):
        import sonos_screen
        self.app.go(sonos_screen.SonosScreen(self.app))


class ComingSoon(Screen):
    def __init__(self, app, title):
        self.title = title
        super().__init__(app)

    def draw(self):
        super().draw()
        self.cv.ctext('Coming soon', W // 2, 170, DIM, 2)


class FacePicker(Screen):
    title = 'Faces'

    def __init__(self, app):
        super().__init__(app)
        import faces
        current = app.settings['face']
        step = min(66, (H - HEADER - 8) // len(faces.FACES))
        for i, (key, name, _) in enumerate(faces.FACES):
            sel = key == current
            self.buttons.append(Button(16, HEADER + 8 + i * step, W - 32, step - 8, name,
                                       lambda k=key: self.pick(k),
                                       ACCENT if sel else PANEL2, BG if sel else FG))

    def pick(self, key):
        self.app.settings['face'] = key
        settings.save(self.app.settings)
        self.app.home()


class SettingsScreen(Screen):
    title = 'Settings'
    SLIDER = (226, 76, W - 226 - 16, 32)
    MIN_BRIGHTNESS = 5          # never let it go fully dark

    def __init__(self, app):
        super().__init__(app)
        dark = app.settings.get('theme', 'dark') == 'dark'
        actions = (
            ('Device', self.device),
            ('Light mode' if dark else 'Dark mode', self.toggle_theme),
            ('Calibrate', lambda: app.go(Calibrate(app))),
            ('Weather', self.refresh_weather),
            ('Sync time', self.sync_time),
        )
        for i, (label, action) in enumerate(actions):
            self.buttons.append(Button(16, HEADER + 12 + i * 50, 190, 44, label, action))

    def draw(self):
        super().draw()
        cv = self.cv
        wx = self.app.weather
        gc.collect()
        level = self.cv.lcd.level
        x, y, w, h = self.SLIDER
        cv.big_text('BRIGHTNESS', x, HEADER + 12, DIM, 1)
        cv.rtext('%d%%' % level, x + w, HEADER + 12, FG, 1)
        cv.round_rect(x, y, w, h, 8, PANEL2)
        cv.round_rect(x, y, max(16, w * level // 100), h, 8, ACCENT)
        rows = (
            ('WI-FI', net.ip() or 'offline'),
            ('LOCATION', wx.city or '-'),
            ('WEATHER UPDATED', wx.updated or 'never'),
            ('FREE RAM', '%dK' % (gc.mem_free() // 1024)),
        )
        y = HEADER + 72
        for label, value in rows:
            cv.big_text(label, 226, y, DIM, 1)
            cv.big_text(value, 226, y + 12, FG, 2)
            y += 50

    def device(self):
        import device_screen
        self.app.go(device_screen.DeviceScreen(self.app))

    def tap(self, x, y):
        sx, sy, sw, sh = self.SLIDER
        if sx - 10 <= x < sx + sw + 10 and sy - 10 <= y < sy + sh + 10:
            level = max(self.MIN_BRIGHTNESS, min(100, (x - sx) * 100 // sw))
            self.cv.lcd.brightness(level)
            self.app.settings['brightness'] = level
            settings.save(self.app.settings)
            self.enter()
            return
        super().tap(x, y)

    def toggle_theme(self):
        s = self.app.settings
        s['theme'] = 'light' if s.get('theme', 'dark') == 'dark' else 'dark'
        settings.save(s)
        # Every screen draws with palette slots, so swapping the palette re-themes everything
        self.cv.set_palette(gfx.THEMES[s['theme']])
        self.app.go(SettingsScreen(self.app))

    def refresh_weather(self):
        self.app.task(self.app.weather.update(), 'Updating weather', lambda r, e: self.enter())

    def sync_time(self):
        def done(ok, _err):
            self.app.time_ok = bool(ok) or self.app.time_ok
            self.enter()
        self.app.task(net.sync_time(), 'Syncing time', done)


class Calibrate(Screen):
    """9-point calibration with an affine fit, which also corrects the panel's
    skew/rotation that a per-axis scale can't."""
    timeout = None
    TARGETS = tuple((x, y) for y in (40, 160, 280) for x in (40, 240, 440))
    MAX_ERR = 25        # px; a worse average fit means a mis-tap

    def __init__(self, app):
        super().__init__(app)
        self.samples = []
        self.message = ''

    def draw(self):
        cv = self.cv
        cv.fill(BG)
        cv.ctext('Touch calibration', W // 2, 88, FG, 2)
        cv.ctext(self.message or 'Tap the centre of each cross', W // 2, 112,
                 ACCENT2 if self.message else DIM, 2)
        cv.ctext('%d / %d' % (len(self.samples) + 1, len(self.TARGETS)), W // 2, 204, DIM, 2)
        x, y = self.TARGETS[len(self.samples)]
        cv.hline(x - 16, y, 33, ACCENT)
        cv.vline(x, y - 16, 33, ACCENT)
        cv.ellipse(x, y, 7, 7, ACCENT2)

    def tap_raw(self, rx, ry):
        self.samples.append((rx, ry))
        if len(self.samples) < len(self.TARGETS):
            self.enter()
            return
        import touch
        cal, err = touch.fit_affine(self.samples, self.TARGETS)
        print('calibration', cal, 'mean error px %.1f' % err)
        if err > self.MAX_ERR:
            self.samples = []
            self.message = 'Off by %dpx - try again' % err
            self.enter()
            return
        self.app.touch.calib = cal
        self.app.settings['calib'] = cal
        settings.save(self.app.settings)
        cv = self.cv
        cv.fill(BG)
        cv.ctext('Calibrated', W // 2, 140, FG, 2)
        cv.ctext('Accuracy about %dpx' % max(1, err), W // 2, 170, DIM, 2)
        cv.flush()
        import time
        time.sleep(1.5)
        self.app.home()
