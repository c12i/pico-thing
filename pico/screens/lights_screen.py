# Yeelight list + per-light controls.
import config
import settings
import yeelight
from gfx import (W, BG, FG, DIM, MUTED, PANEL, PANEL2, WHITE, ACCENT,
                 RED, GREEN, BLUE, YELLOW, ORANGE, PURPLE)
from ui import Screen, Button, HEADER

CT_PRESETS = (('Warm', 2700, ORANGE), ('Soft', 3500, YELLOW), ('Day', 4500, WHITE), ('Cool', 6500, ACCENT))
RGB_PRESETS = ((0xFF0000, RED), (0xFF7A00, ORANGE), (0x00FF40, GREEN), (0x0040FF, BLUE), (0x8A00FF, PURPLE))
BRIGHT_PRESETS = (10, 25, 50, 75, 100)


def with_config(lights):
    """Add miIO lights from config (they can't be discovered) and apply names."""
    tokens = getattr(config, 'MIIO_TOKENS', {})
    if tokens:
        import miio
        known = [l.ip for l in lights]
        lights = lights + [miio.MiioLight(ip, tok) for ip, tok in tokens.items() if ip not in known]
    names = getattr(config, 'LIGHT_NAMES', {})
    for l in lights:
        l.name = names.get(l.ip) or names.get(l.id) or l.name
    return lights


def load_lights(app):
    return with_config([yeelight.Light.from_dict(d) for d in app.settings.get('lights', ())])


def save_lights(app, lights):
    # Only discovered lights are cached; miIO ones always come from config
    app.settings['lights'] = [l.to_dict() for l in lights if not l.id.startswith('miio:')]
    settings.save(app.settings)


def status_text(light):
    if light.online is False:
        return 'Offline'
    return ('On  %d%%' % light.bright) if light.power else 'Off'


def power_button(x, y, w, h, light, action):
    on = light.power
    return Button(x, y, w, h, 'ON' if on else 'OFF', action, YELLOW if on else PANEL2, BG if on else FG)


def chip_row(y, h, items):
    """items: (label, color, fg, action); returns evenly spaced buttons across the screen."""
    n, gap = len(items), 8
    w = (W - 32 - gap * (n - 1)) // n
    return [Button(16 + i * (w + gap), y, w, h, label, action, color, fg)
            for i, (label, color, fg, action) in enumerate(items)]


class LightRow(Button):
    def __init__(self, x, y, w, h, light, action):
        super().__init__(x, y, w, h, light.label, action, PANEL)
        self.light = light

    def draw(self, cv, pressed=False):
        cv.round_rect(self.x, self.y, self.w, self.h, 10, PANEL2 if pressed else PANEL)
        l = self.light
        cv.big_text(l.label, self.x + 16, self.y + 12, FG, 2)
        status = status_text(l)
        cv.big_text(status, self.x + 16, self.y + 38, RED if l.online is False else ACCENT, 1)
        cv.big_text('%s  %s' % (l.model, l.ip), self.x + 16 + 8 * (len(status) + 2), self.y + 38, DIM, 1)
        cv.big_text('>', self.x + self.w - 28, self.y + self.h // 2 - 8, MUTED, 2)


class LightsScreen(Screen):
    feature = 'lights'
    title = 'Lights'

    def __init__(self, app):
        super().__init__(app)
        self.lights = load_lights(app)
        self.message = ''
        self._build()

    def _build(self):
        app = self.app
        self.buttons = self.buttons[:1]     # keep the back button
        self.buttons.append(Button(W - 132, 6, 120, 36, 'All off', self.all_off, PANEL2, FG, 2))
        for i, l in enumerate(self.lights[:3]):
            y = HEADER + 12 + i * 68
            self.buttons.append(LightRow(12, y, 330, 60, l, lambda l=l: app.go(LightDetail(app, l))))
            self.buttons.append(power_button(352, y, 116, 60, l, lambda l=l: self.toggle(l)))
        self.buttons.append(Button(12, 262, W - 24, 48, 'Scan for lights', self.scan, PANEL2, DIM, 2))

    def draw(self):
        super().draw()
        if self.message:
            self.cv.ctext(self.message, W // 2, 150 if not self.lights else 250, DIM if not self.lights else RED, 1 if self.lights else 2)

    def _redraw(self, _res=None, err=None):
        if err:
            self.message = str(err)
        self._build()
        Screen.enter(self)

    def enter(self):
        super().enter()
        if not self.lights:
            self.scan()
        else:
            self.app.task(self._refresh_all(), 'Checking lights', self._redraw)

    async def _refresh_all(self):
        for l in self.lights:
            try:
                await l.refresh()
            except Exception:
                pass

    def scan(self):
        self.message = ''
        self.app.task(self._scan(), 'Scanning for lights', self._redraw)

    async def _scan(self):
        found = await yeelight.discover(3)
        save_lights(self.app, found)
        self.lights = with_config(found)
        for l in self.lights:
            if l.online is None:
                try:
                    await l.refresh()
                except Exception:
                    pass
        self.message = '' if self.lights else 'No lights found - is LAN Control on?'

    def toggle(self, light):
        self.message = ''
        self.app.task(light.set_power(not light.power),
                      'Turning %s %s' % (light.label, 'off' if light.power else 'on'), self._redraw)

    def all_off(self):
        async def off():
            for l in self.lights:
                try:
                    await l.set_power(False)
                except Exception:
                    pass
        self.message = ''
        self.app.task(off(), 'Turning all off', self._redraw)


class LightDetail(Screen):
    feature = 'lights'
    SLIDER = (16, 120, W - 32, 44)

    def __init__(self, app, light):
        self.title = light.label
        super().__init__(app)
        self.light = light
        self.message = ''
        self._build()

    def back(self):
        self.app.go(LightsScreen(self.app))

    def _build(self):
        l = self.light
        self.buttons = self.buttons[:1]
        self.buttons.append(power_button(16, 56, 140, 56, l, lambda: self._do(
            l.set_power(not l.power), 'Turning off' if l.power else 'Turning on')))
        if l.can('start_cf'):
            self.buttons.append(Button(W - 156, 56, 140, 56, 'Identify',
                                       lambda: self._do(l.identify(), 'Blinking')))
        y = 172
        self.buttons += chip_row(y, 40, [('%d%%' % p, PANEL2, FG,
                                          lambda p=p: self._do(l.set_bright(p), 'Brightness %d%%' % p))
                                         for p in BRIGHT_PRESETS])
        if l.can('set_ct_abx'):
            y += 48
            self.buttons += chip_row(y, 40, [(name, color, BG, lambda k=k, n=name: self._do(l.set_ct(k), n))
                                             for name, k, color in CT_PRESETS])
        if l.can('set_rgb'):
            y += 48
            self.buttons += chip_row(y, 40, [('', color, BG, lambda v=v: self._do(l.set_rgb(v), 'Colour'))
                                             for v, color in RGB_PRESETS])

    def draw(self):
        super().draw()
        cv, l = self.cv, self.light
        if l.online is False:
            cv.big_text('Offline', 172, 66, RED, 2)
        else:
            cv.big_text(status_text(l), 172, 66, FG, 2)
        info = '%s  %s' % (l.model, l.ip)
        if l.ct and l.can('set_ct_abx'):
            info += '  %dK' % l.ct
        cv.big_text(info, 172, 92, DIM, 1)
        x, y, w, h = self.SLIDER
        cv.round_rect(x, y, w, h, 10, PANEL2)
        if l.power and l.bright:
            cv.round_rect(x, y, max(20, w * l.bright // 100), h, 10, YELLOW)
        cv.ctext('Brightness - tap to set', W // 2, y + h // 2 - 4, BG if l.power and l.bright > 60 else FG, 1)
        if self.message:
            cv.ctext(self.message[:56], W // 2, 166, RED, 1)

    def tap(self, x, y):
        sx, sy, sw, sh = self.SLIDER
        if sx <= x < sx + sw and sy <= y < sy + sh:
            pct = (x - sx) * 100 // sw + 1
            self._do(self.light.set_bright(pct), 'Brightness %d%%' % pct)
            return
        super().tap(x, y)

    def _do(self, coro, label):
        self.message = ''
        self.app.task(coro, label, self._done)

    def _done(self, _res, err):
        self.message = str(err) if err else ''
        self._build()
        Screen.enter(self)
