# TODOs from the Mac, a Pomodoro status strip, a robot that reacts to both, and
# a rotating quote.
import json
import time
import bot
import config
from gfx import wrap, W, H, BG, FG, DIM, PANEL, ACCENT
from faces import StripFace

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
