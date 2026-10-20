# Home screen faces. Tap anywhere to open the menu.
import json
import math
import time
import bot
import config
import icons
from gfx import (wrap, W, H, BG, FG, DIM, MUTED, PANEL, ACCENT, ACCENT2, GREEN, GREEN2, YELLOW,
                 ORANGE, DASH)
from ui import Screen

DAYS = ('MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN')
MONTHS = ('JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC')


def deg(v):
    return '--' if v is None else '%d' % round(v)


def date_str(t):
    return '%s %d %s' % (DAYS[t[6]], t[2], MONTHS[t[1] - 1])


class Face(Screen):
    timeout = None
    is_face = True      # agent attention prompts may take over faces

    def enter(self):
        t = self.app.localtime()
        self._day = t[2]
        self.draw()
        self.update(t, True)
        self.cv.flush()

    def second(self, t):
        if t[2] != self._day:        # new day (or the clock just got synced)
            self.enter()
            return
        r = self.update(t, False)
        if r:
            self.cv.flush(*r)

    def tap(self, x, y):
        self.app.menu()

    def on_weather(self):
        self.enter()

    def update(self, t, full):
        """Redraw the time-dependent part; return the rect to flush, if any."""
        return None


class DigitalFace(Face):
    DW, DH, T, GAP, CW = 78, 140, 16, 14, 28

    def draw(self):
        cv = self.cv
        cv.fill(BG)
        cv.round_rect(12, 238, W - 24, 70, 14, PANEL)
        wx = self.app.weather
        if not wx.ok:
            cv.ctext('Weather unavailable', W // 2, 266, DIM, 2)
            return
        icons.draw(cv, wx.icon, 52, 273, 54, PANEL)
        cv.deg_text(deg(wx.temp), 92, 261, FG, 3)
        cv.big_text(wx.label, 196, 252, FG, 2)
        cv.big_text('H %s  L %s' % (deg(wx.hi), deg(wx.lo)), 196, 278, DIM, 2)

    def update(self, t, full):
        cv = self.cv
        dw, dh, gap = self.DW, self.DH, self.GAP
        total = 4 * dw + 4 * gap + self.CW
        x, y = (W - total) // 2, 22
        cv.fill_rect(0, 0, W, 232, BG)
        if self.app.time_ok:
            digits = (t[3] // 10, t[3] % 10, t[4] // 10, t[4] % 10)
        else:
            digits = (DASH,) * 4
        colon = x + 2 * dw + 2 * gap
        xs = (x, x + dw + gap, colon + self.CW + gap, colon + self.CW + 2 * gap + dw)
        for d, dx in zip(digits, xs):
            cv.digit(d, dx, y, dw, dh, self.T, FG)
        for cy in (y + dh * 3 // 10, y + dh * 7 // 10 - self.T):
            cv.fill_rect(colon + (self.CW - self.T) // 2, cy, self.T, self.T, ACCENT)
        bar_y = y + dh + 14
        cv.fill_rect(x, bar_y, total, 4, MUTED)
        cv.fill_rect(x, bar_y, total * t[5] // 59, 4, ACCENT)
        cv.ctext(date_str(t), W // 2, bar_y + 20, ACCENT, 3)
        return 0, 0, W, 232


class AnalogFace(Face):
    CX, CY, R = 160, 160, 148

    def __init__(self, app):
        super().__init__(app)
        self.trig = [(math.sin(i * math.pi / 30), -math.cos(i * math.pi / 30)) for i in range(60)]

    def draw(self):
        cv = self.cv
        cv.fill(BG)
        t = self.app.localtime()
        cx = 400
        cv.ctext(DAYS[t[6]], cx, 28, DIM, 2)
        cv.ctext('%d' % t[2], cx, 54, FG, 6)
        cv.ctext(MONTHS[t[1] - 1], cx, 112, DIM, 2)
        cv.hline(340, 146, 120, MUTED)
        wx = self.app.weather
        if not wx.ok:
            cv.ctext('No weather', cx, 200, DIM, 1)
            return
        icons.draw(cv, wx.icon, cx, 196, 72)
        s = deg(wx.temp)
        cv.deg_text(s, cx - cv.deg_width(s, 3) // 2, 244, FG, 3)
        cv.ctext(wx.label, cx, 280, DIM, 1)
        cv.ctext('H %s  L %s' % (deg(wx.hi), deg(wx.lo)), cx, 296, DIM, 1)

    def _hand(self, angle, length, width, color, tail=0):
        a = math.radians(angle)
        s, c = math.sin(a), -math.cos(a)
        cx, cy = self.CX, self.CY
        self.cv.thick_line(int(cx - s * tail), int(cy - c * tail),
                           int(cx + s * length), int(cy + c * length), width, color)

    def update(self, t, full):
        cv = self.cv
        cx, cy, r = self.CX, self.CY, self.R
        cv.fill_rect(0, 0, 320, H, BG)
        cv.ellipse(cx, cy, r, r, PANEL, True)
        for i, (s, c) in enumerate(self.trig):
            if i % 5:
                cv.fill_rect(int(cx + s * (r - 9)) - 1, int(cy + c * (r - 9)) - 1, 2, 2, MUTED)
            else:
                cv.thick_line(int(cx + s * (r - 24)), int(cy + c * (r - 24)),
                              int(cx + s * (r - 6)), int(cy + c * (r - 6)),
                              4, FG if i % 15 == 0 else DIM)
        if self.app.time_ok:
            h, m, sec = t[3], t[4], t[5]
            self._hand((h % 12 + m / 60) * 30, r * 0.5, 9, FG)
            self._hand((m + sec / 60) * 6, r * 0.78, 6, FG)
            self._hand(sec * 6, r * 0.86, 2, ACCENT2, r * 0.16)
        else:
            cv.ctext('--:--', cx, cy + 40, DIM, 2)
        cv.ellipse(cx, cy, 7, 7, ACCENT2, True)
        cv.ellipse(cx, cy, 3, 3, BG, True)
        return 0, 0, 320, H


class WeatherFace(Face):
    def draw(self):
        cv = self.cv
        cv.fill(BG)
        self._min = -1
        wx = self.app.weather
        if not wx.ok:
            cv.ctext('Weather unavailable', W // 2, 150, DIM, 2)
            return
        icons.draw(cv, wx.icon, 120, 148, 150)
        cv.deg_text(deg(wx.temp), 250, 66, FG, 8)
        cv.big_text(wx.label, 252, 146, FG, 2)
        cv.big_text('H %s  L %s' % (deg(wx.hi), deg(wx.lo)), 252, 174, DIM, 2)
        cv.big_text(wx.city[:13], 252, 202, DIM, 2)

        cv.round_rect(12, 238, W - 24, 70, 14, PANEL)
        wind = 'mph' if wx.fahrenheit else 'km/h'
        cols = (('FEELS', deg(wx.feels), True),
                ('HUMIDITY', '%s%%' % deg(wx.humidity), False),
                ('WIND', deg(wx.wind) + wind, False),
                ('RAIN', '%s%%' % deg(wx.rain), False))
        cw = (W - 24) // 4
        for i, (label, value, is_deg) in enumerate(cols):
            cx = 12 + i * cw + cw // 2
            cv.ctext(label, cx, 252, DIM, 1)
            if is_deg:
                cv.deg_text(value, cx - cv.deg_width(value, 2) // 2, 272, FG, 2)
            else:
                cv.ctext(value, cx, 272, FG, 2)

    def update(self, t, full):
        if t[4] == self._min:
            return None
        self._min = t[4]
        cv = self.cv
        cv.fill_rect(0, 0, W, 48, BG)
        hm = '%02d:%02d' % (t[3], t[4]) if self.app.time_ok else '--:--'
        cv.big_text(hm, 16, 14, FG, 3)
        cv.rtext(date_str(t), W - 16, 18, DIM, 2)
        return 0, 0, W, 48


DEFAULT_QUOTES = (
    'Lock in.',
    'Avoid distractions. Do the work.',
    'Keep learning.',
    'Have you learnt something today?',
    'Slow improvement is still improvement.',
    'Keep going.',
    '1% better every day.',
    'Deep work beats busy work.',
    'Small steps, every single day.',
    'Discipline over motivation.',
    'Done is better than perfect.',
    'One thing at a time.',
    'Ship it, then improve it.',
    'Consistency compounds.',
    'Progress, not perfection.',
    'Stay curious.',
    'Read the docs. Then the code.',
    'Rest is part of the work.',
    'Future you will thank you.',
    'Hard things are learnable.',
)
# config.QUOTES overrides the built-in list (config.py isn't frozen, so no rebuild needed)
QUOTES = tuple(getattr(config, 'QUOTES', None) or DEFAULT_QUOTES)


class StripFace(Face):
    """A face with the Productivity top strip: time, date, and the Pomodoro status
    (or the weather when no timer is running). Tap the strip to open the timer."""
    STRIP = 62

    def __init__(self, app):
        super().__init__(app)
        self._min = -1
        self._pstate = None

    def strip(self, t, full):
        """Redraw the strip if anything in it changed; True if it did."""
        pomo = self.app.pomo
        # The timer ticks every second; otherwise only redraw on the minute
        if not full and t[4] == self._min and pomo.state != 'running' and pomo.state == self._pstate:
            return False
        self._min, self._pstate = t[4], pomo.state
        cv = self.cv
        cv.fill_rect(0, 0, W, self.STRIP, BG)
        cv.big_text('%02d:%02d' % (t[3], t[4]) if self.app.time_ok else '--:--', 16, 12, FG, 4)
        cv.rtext(date_str(t), W - 16, 12, DIM, 2)
        cv.hline(16, 58, W - 32, MUTED)
        if pomo.active:
            left = pomo.remaining()
            color = ORANGE if pomo.state == 'done' else ACCENT if pomo.phase == 'focus' else GREEN
            text = ('%s done' % ('FOCUS' if pomo.finished == 'focus' else 'BREAK') if pomo.state == 'done'
                    else '%s %02d:%02d%s' % ('FOCUS' if pomo.phase == 'focus' else 'BREAK',
                                             left // 60, left % 60,
                                             ' II' if pomo.state == 'paused' else ''))
            cv.rtext(text, W - 16, 34, color, 2)
            cv.fill_rect(16, 57, int((W - 32) * pomo.progress()), 3, color)
        else:
            wx = self.app.weather
            if wx.ok:
                cv.rtext('%s  %sC' % (wx.label, deg(wx.temp)), W - 16, 38, DIM, 1)
        return True

    def tap(self, x, y):
        if y < self.STRIP:
            import pomodoro_screen
            self.app.go(pomodoro_screen.PomodoroScreen(self.app))
        else:
            self.app.menu()


class ProductivityFace(StripFace):
    """Time, Pomodoro status, open TODOs (synced from the Mac companion) and a
    rotating quote. Tap the top strip to open the timer."""
    ROWS = 6
    ITEM_CHARS = 20          # leaves the right side for the bot
    LIST_CX = 186            # centre of the list column
    BOT_CX, BOT_TOP = 420, 64
    OVERWHELMED = 6          # this many open items and the bot starts to panic
    EXCITED_S = 60           # how long it celebrates a ticked-off item
    NIGHT = (23, 6)
    # Marquee for items longer than ITEM_CHARS: hold, scroll to the end, hold, restart
    SCROLL_PX, SCROLL_MS, HOLD_MS = 4, 200, 2000
    QUOTE_S = 300           # new quote every 5 minutes
    TODO_REFRESH_S = 300
    CACHE = 'todos.json'

    def __init__(self, app):
        super().__init__(app)
        self._qi = -1
        self._since = 0
        self._rev = None
        self._frame = 0
        self._excited_until = 0
        self._offline = False
        self._marquee = []          # TODO rows: [x, y, chars, text, span_px, offset, t0, bg]
        self._quote_mq = None       # same shape, for a quote too long for two lines
        self._mq_last = 0
        try:
            with open(self.CACHE) as f:
                self.todos = json.load(f)
        except (OSError, ValueError):
            self.todos = None

    def enter(self):
        super().enter()
        self._fetch()

    def draw(self):
        cv = self.cv
        cv.fill(BG)
        self._qi = -1                      # force the quote to draw
        self._draw_todos()

    def _draw_todos(self):
        cv = self.cv
        cv.fill_rect(0, 62, W, 180, BG)
        items = self.todos or []
        cv.big_text('TODO', 16, 68, ACCENT, 1)
        self._marquee = []
        self._draw_bot()
        if self.todos is None:
            cv.ctext('Waiting for Mac...', self.LIST_CX, 140, DIM, 2)
            return
        cv.big_text('%d open' % len(items), 60, 68, DIM, 1)
        if not items:
            cv.ctext('All clear. Nice.', self.LIST_CX, 140, DIM, 2)
            return
        n = self.ITEM_CHARS
        shown = items[:self.ROWS] if len(items) <= self.ROWS else items[:self.ROWS - 1]
        for k, item in enumerate(shown):
            y = 84 + k * 26
            cv.rect(18, y + 2, 12, 12, DIM)
            cv.big_text(item[:n], 40, y, FG, 2)
            if len(item) > n:
                self._marquee.append(self._scroller(40, y, n, item, BG))
        if len(items) > len(shown):
            cv.big_text('+%d more' % (len(items) - len(shown)), 40, 84 + len(shown) * 26 + 4, DIM, 1)

    def _scroller(self, x, y, chars, text, bg):
        return [x, y, chars, text, (len(text) - chars) * 16, 0, time.ticks_ms(), bg]

    def frame(self, now):
        if time.ticks_diff(now, self._mq_last) < self.SCROLL_MS:
            return
        self._mq_last = now
        for m in self._marquee + ([self._quote_mq] if self._quote_mq else []):
            self._step(m, now)

    def _step(self, m, now):
        x, y, n, text, span, last, t0, bg = m
        speed = self.SCROLL_PX / self.SCROLL_MS                  # px per ms
        t = time.ticks_diff(now, t0) % (2 * self.HOLD_MS + int(span / speed))
        off = 0 if t < self.HOLD_MS else min(span, int((t - self.HOLD_MS) * speed))
        if off == last:
            return
        m[5] = off
        cv = self.cv
        i, px = off // 16, off % 16
        cv.fill_rect(x - 8, y, n * 16 + 8, 16, bg)
        cv.big_text(text[i:i + n], x - px, y, FG, 2)
        cv.fill_rect(x - 8, y, 8, 16, bg)                        # clip what slid past the left edge
        cv.flush(x - 8, y, n * 16 + 8, 16)

    def mood(self):
        pomo = self.app.pomo
        if pomo.state == 'running':
            return 'busy' if pomo.phase == 'focus' else 'sleeping'
        if pomo.state == 'paused':
            return 'bored'
        if time.time() < self._excited_until or (pomo.state == 'done' and pomo.finished == 'focus'):
            return 'excited'
        if self.todos is None or self._offline:
            return 'confused'
        hour = self.app.localtime()[3]
        if self.app.time_ok and (hour >= self.NIGHT[0] or hour < self.NIGHT[1]):
            return 'sleeping'
        if len(self.todos) >= self.OVERWHELMED:
            return 'overwhelmed'
        if not self.todos:
            return 'chill'
        return 'happy'

    def _draw_bot(self):
        bot.draw(self.cv, self.mood(), self._frame, self.BOT_CX, self.BOT_TOP)

    def _draw_quote(self):
        cv = self.cv
        cv.fill_rect(0, 244, W, H - 244, BG)
        cv.round_rect(12, 246, W - 24, 62, 12, PANEL)
        quote = QUOTES[self._qi]
        self._quote_mq = None
        lines = wrap(quote, 26)
        if len(lines) > 2:                 # too long for two lines: one scrolling line
            cv.big_text(quote[:26], 32, 269, FG, 2)
            self._quote_mq = self._scroller(32, 269, 26, quote, PANEL)
            return
        y = 246 + (62 - len(lines) * 24) // 2 + 4
        for k, line in enumerate(lines):
            cv.ctext(line, W // 2, y + k * 24, FG, 2)

    def update(self, t, full):
        if not self.strip(t, full):
            return None
        qi = (time.time() // self.QUOTE_S) % len(QUOTES)
        if qi != self._qi:
            self._qi = qi
            self._draw_quote()
            return 0, 0, W, H
        return 0, 0, W, self.STRIP

    def second(self, t):
        super().second(t)
        if self.app.screen is not self:          # e.g. the day rolled over and we re-entered
            return
        self._frame += 1
        self._draw_bot()
        self.cv.flush(self.BOT_CX - bot.BOX_W // 2, self.BOT_TOP, bot.BOX_W, bot.BOX_H)
        self._since += 1
        if self._since >= self.TODO_REFRESH_S:
            self._since = 0
            self._fetch()

    def on_companion(self):
        # The companion bumps todos_rev (and pokes us) whenever the TODO file is saved
        rev = (getattr(self.app, 'companion', None) or {}).get('todos_rev')
        if rev != self._rev:
            self._rev = rev
            self._fetch()

    def _fetch(self):
        import companion
        if companion.BASE:
            self.app.background(companion.call('GET', '/todos'), self._got)

    def _got(self, res, err):
        self._offline = bool(err)
        if err or not res:
            return
        items = res.get('items', [])
        if items == self.todos:
            return
        if self.todos is not None and len(items) < len(self.todos):
            self._excited_until = time.time() + self.EXCITED_S    # something got ticked off
        self.todos = items
        with open(self.CACHE, 'w') as f:
            json.dump(items, f)
        self._draw_todos()
        self.cv.flush(0, 62, W, 180)


class BonsaiFace(StripFace):
    """The top strip over a growing bonsai: cbonsai --live --infinite --life=16
    --time=0.1, drawn on a grid of terminal-like character cells."""
    CW, CH = 10, 18                     # cell size; glyphs are the 8x8 font stretched to 8x16
    COLS, ROWS = W // 10, 14
    TOP = H - 14 * 18
    LIFE = 16
    STEP_MS = 100                       # --time=0.1
    WAIT_MS = 4000                      # cbonsai's default --wait before regrowing
    # bonsai styles -> colours (cbonsai's bold yellow/yellow wood, green/bright green leaves)
    COLORS = (YELLOW, ORANGE, GREEN, GREEN2, GREEN, GREEN)

    def __init__(self, app):
        super().__init__(app)
        import framebuf
        self._glyph = framebuf.FrameBuffer(bytearray(8), 8, 8, framebuf.MONO_HLSB)
        self._t = 0
        self._done = None

    def draw(self):
        import bonsai
        self.cv.fill(BG)
        rows = self.ROWS - bonsai.BASE_H
        bx = self.COLS // 2 - bonsai.BASE_W // 2
        for r, runs in enumerate(bonsai.BASE):
            x = bx
            for text, style in runs:
                self._put(rows + r, x, text, style)
                x += len(text)
        self._grow()

    def on_weather(self):
        if self.strip(self.app.localtime(), True):
            self.cv.flush(0, 0, W, self.STRIP)

    def _grow(self):
        """Clear the old tree (the pot stays) and plant a new one."""
        import bonsai
        rows = self.ROWS - bonsai.BASE_H
        self.cv.fill_rect(0, self.TOP, W, rows * self.CH, BG)
        self.tree = bonsai.Tree(rows, self.COLS, self.LIFE)
        self._done = None

    def _put(self, row, col, text, style):
        cv, g, px = self.cv, self._glyph, self._glyph.pixel
        c = DIM if style is None else self.COLORS[style]
        y = self.TOP + row * self.CH
        for i, ch in enumerate(text):
            if row < 0 or not 0 <= col + i < self.COLS:
                continue
            x = (col + i) * self.CW
            cv.fill_rect(x, y, self.CW, self.CH, BG)
            if ch == ' ':
                continue
            g.fill(0)
            g.text(ch, 0, 0, 1)
            for r in range(8):
                k = 0
                while k < 8:
                    if px(k, r):
                        start = k
                        while k < 8 and px(k, r):
                            k += 1
                        cv.fill_rect(x + 1 + start, y + 1 + 2 * r, k - start, 2, c)
                    else:
                        k += 1

    def update(self, t, full):
        return (0, 0, W, self.STRIP) if self.strip(t, full) else None

    def frame(self, now):
        if time.ticks_diff(now, self._t) < self.STEP_MS:
            return
        self._t = now
        if self._done is not None:                      # --infinite: wait, then a new tree
            if time.ticks_diff(now, self._done) >= self.WAIT_MS:
                self._grow()
                self.cv.flush(0, self.TOP, W, self.tree.rows * self.CH)
            return
        s = self.tree.step()
        if s is None:
            self._done = now
            return
        y, x, text, style = s
        self._put(y, x, text, style)
        self.cv.flush(x * self.CW, self.TOP + y * self.CH, len(text) * self.CW, self.CH)


FACES = (
    ('digital', 'Digital', DigitalFace),
    ('analog', 'Analog', AnalogFace),
    ('weather', 'Weather', WeatherFace),
    ('productivity', 'Productivity', ProductivityFace),
    ('bonsai', 'Bonsai', BonsaiFace),
)


def make(app, key):
    key = {'focus': 'productivity'}.get(key, key)     # renamed
    for k, _, cls in FACES:
        if k == key:
            return cls(app)
    return FACES[0][2](app)
