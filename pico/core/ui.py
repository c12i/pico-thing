# Tiny screen/button framework and the async main loop.
import asyncio
import gc
import math
import sys
import time
from gfx import W, BG, FG, DIM, MUTED, PANEL, PANEL2, ACCENT

HEADER = 48
SNAP = 22     # taps this close to a button still count (fingers on a resistive panel land ~20px off)


class Button:
    def __init__(self, x, y, w, h, label, action, color=PANEL2, fg=FG, scale=2):
        self.x, self.y, self.w, self.h = x, y, w, h
        self.label = label
        self.action = action
        self.color = color
        self.fg = fg
        self.scale = scale

    def hit(self, x, y):
        return self.x <= x < self.x + self.w and self.y <= y < self.y + self.h

    def distance(self, x, y):
        dx = max(self.x - x, 0, x - (self.x + self.w - 1))
        dy = max(self.y - y, 0, y - (self.y + self.h - 1))
        return max(dx, dy)

    def draw(self, cv, pressed=False):
        cv.round_rect(self.x, self.y, self.w, self.h, 10, ACCENT if pressed else self.color)
        cv.ctext(self.label, self.x + self.w // 2, self.y + (self.h - 8 * self.scale) // 2,
                 BG if pressed else self.fg, self.scale)


class Screen:
    title = None
    timeout = 30      # seconds idle before falling back to the home face; None = never
    feature = None    # key into FEATURES: modules to keep loaded while this screen is up

    def __init__(self, app):
        self.app = app
        self.cv = app.cv
        self.buttons = []
        if self.title:
            self.buttons.append(Button(0, 0, 64, HEADER, '<', self.back, PANEL, FG, 3))

    def back(self):
        self.app.menu()

    def enter(self):
        self.draw()
        self.cv.flush()

    def draw(self):
        cv = self.cv
        cv.fill(BG)
        if self.title:
            cv.fill_rect(0, 0, W, HEADER, PANEL)
            cv.big_text(self.title, 76, (HEADER - 16) // 2, FG, 2)
        for b in self.buttons:
            b.draw(cv)

    def second(self, t):
        """Called once a second with the local time tuple."""

    def button_at(self, x, y):
        """The button under (x, y), else the nearest one within SNAP pixels."""
        best, best_d = None, SNAP + 1
        for b in self.buttons:
            d = b.distance(x, y)
            if d < best_d:
                best, best_d = b, d
        return best

    def tap(self, x, y):
        b = self.button_at(x, y)
        if not b:
            return
        b.draw(self.cv, True)
        self.cv.flush(b.x, b.y, b.w, b.h)
        time.sleep_ms(60)
        b.action()
        if self.app.screen is self and b in self.buttons:
            b.draw(self.cv)
            self.cv.flush(b.x, b.y, b.w, b.h)


# Loading box for foreground tasks, and the corner dot for background ones
OVERLAY = (100, 130, 280, 60)    # must fit gfx.SCRATCH
SPINNER = (100 + 12, 130 + 10, 40, 40)   # the part that animates, inside OVERLAY

# Module code lives in RAM (~65KB for everything), so feature screens and their
# device drivers are unloaded whenever you're not on them
FEATURES = {
    'lights': ('lights_screen', 'yeelight', 'miio'),
    'sonos': ('sonos_screen', 'sonos'),
    'device': ('device_screen',),
    'command': ('command_screen',),
    'pomodoro': ('pomodoro_screen',),
}
DOT = (W - 10, 4, 6, 6)
SPIN_MS = 110
LOOP_MS = 15


class App:
    def __init__(self, cv, touch, weather, settings):
        self.cv = cv
        self.touch = touch
        self.weather = weather
        self.settings = settings
        self.screen = None
        self.time_ok = False
        self.tasks = []
        self.last_input = time.ticks_ms()
        self.busy = None             # label of the running foreground task, if any
        self.background_jobs = 0
        self.net_lock = asyncio.Lock()   # one network conversation at a time
        self._spin = 0
        self._spin_t = 0
        self._dot_on = False
        self._down = False
        self._hold = 0
        self._up = 0
        self._sec = -1
        self._busy_us = 0
        self._win_t0 = time.ticks_us()

    def cpu_load(self):
        """Percent of time spent working (not idle) since the last call."""
        now = time.ticks_us()
        window = time.ticks_diff(now, self._win_t0) or 1
        load = min(100, self._busy_us * 100 // window)
        self._busy_us, self._win_t0 = 0, now
        return load

    def localtime(self):
        return time.localtime(time.time() + self.weather.offset)

    def go(self, screen):
        self.screen = screen
        self.last_input = time.ticks_ms()
        self._unload(screen.feature)
        screen.enter()

    def _unload(self, keep):
        for name, mods in FEATURES.items():
            if name != keep:
                for m in mods:
                    sys.modules.pop(m, None)
        if keep != 'sonos' and hasattr(self, 'sonos'):
            del self.sonos          # speaker objects hold the sonos module alive
        gc.collect()

    def home(self):
        import faces
        self.go(faces.make(self, self.settings['face']))

    def menu(self):
        import screens
        self.go(screens.Launcher(self))

    def every(self, seconds, fn):
        self.tasks.append([seconds * 1000, time.ticks_ms(), fn])

    def status(self, msg):
        cv = self.cv
        cv.fill(BG)
        cv.ctext('pico-thing', W // 2, 120, ACCENT, 3)
        cv.ctext(msg, W // 2, 172, FG, 2)
        cv.flush()

    # --- network tasks ---

    def task(self, coro, label='Loading', done=None):
        """Run a network task the user asked for. Shows a loading box and ignores taps
        until it finishes. done(result, error) is called afterwards if the user is
        still on the same screen."""
        if self.busy:
            coro.close()
            return
        self.busy = label
        asyncio.create_task(self._foreground(coro, done))

    async def _foreground(self, coro, done):
        screen = self.screen
        self._draw_overlay()
        res = err = None
        try:
            async with self.net_lock:
                res = await coro
        except Exception as e:
            sys.print_exception(e)
            err = e
        self.busy = None
        self.cv.flush(*OVERLAY)          # the canvas never had the box drawn into it
        self.last_input = time.ticks_ms()
        if done and self.screen is screen:
            done(res, err)

    def background(self, coro, done=None):
        """Network work the user didn't ask for (periodic refreshes): a small corner
        dot instead of the loading box, and taps keep working."""
        asyncio.create_task(self._background(coro, done))

    async def _background(self, coro, done):
        screen = self.screen
        self.background_jobs += 1
        res = err = None
        try:
            async with self.net_lock:
                res = await coro
        except Exception as e:
            print('background task failed:', e)
            err = e
        self.background_jobs -= 1
        if done and self.screen is screen and not self.busy:
            done(res, err)

    def set_busy_label(self, label):
        """Update the loading box text (e.g. progress) while a task runs."""
        if self.busy:
            self.busy = label
            self._draw_overlay()

    def _draw_overlay(self, spinner_only=False):
        # Drawn into the canvas, pushed to the panel, then the canvas is restored, so
        # screens never see it and clearing it is just re-flushing that region.
        # Each animation frame only repaints the small spinner square.
        cv = self.cv
        x, y, w, h = SPINNER if spinner_only else OVERLAY
        cv.save(x, y, w, h, cv.scratch)
        if not spinner_only:
            cv.round_rect(x, y, w, h, 14, ACCENT)
            cv.round_rect(x + 2, y + 2, w - 4, h - 4, 12, PANEL2)
            label = self.busy or ''
            room = w - 72
            scale = 2 if len(label) * 16 <= room else 1
            cv.big_text(label[:room // (8 * scale)], x + 60, y + h // 2 - 4 * scale, FG, scale)
        sx, sy, sw, sh = SPINNER
        cv.fill_rect(sx, sy, sw, sh, PANEL2)
        cx, cy = sx + sw // 2, sy + sh // 2
        for i in range(8):
            a = i * math.pi / 4
            age = (self._spin - i) % 8
            c = FG if age == 0 else DIM if age < 3 else MUTED
            cv.ellipse(int(cx + math.sin(a) * 14), int(cy - math.cos(a) * 14), 3, 3, c, True)
        cv.flush(x, y, w, h)
        cv.restore(x, y, w, h, cv.scratch)

    def _animate(self, now):
        if time.ticks_diff(now, self._spin_t) < SPIN_MS:
            return
        self._spin_t = now
        if self.busy:
            self._spin = (self._spin + 1) % 8
            self._draw_overlay(spinner_only=True)
        if self.background_jobs:
            self._dot_on = not self._dot_on
            if self._dot_on:
                self.cv.lcd.fill_rect(*DOT, self.cv.color565(ACCENT))
            else:
                self.cv.flush(*DOT)
        elif self._dot_on:
            self._dot_on = False
            self.cv.flush(*DOT)

    # --- input + main loop ---

    def _poll_touch(self):
        r = self.touch.raw()
        if r is None:
            self._up += 1
            if self._up >= 3:        # a few clear reads before we call it released
                self._down = False
                self._hold = 0
            return
        self._up = 0
        if self._down:
            return
        self._hold += 1
        if self._hold < 2:           # ignore the first, noisy contact sample
            return
        self._down = True
        if self.busy:                # swallow taps while a task runs
            return
        self.last_input = time.ticks_ms()
        s = self.screen
        if hasattr(s, 'tap_raw'):
            s.tap_raw(*r)
        else:
            p = self.touch.map(r)
            if p:
                s.tap(*p)

    async def run(self):
        while True:
            t0 = time.ticks_us()
            try:
                self._poll_touch()
                now = time.ticks_ms()
                for t in self.tasks:
                    if time.ticks_diff(now, t[1]) >= t[0]:
                        t[1] = now
                        t[2]()
                if not self.busy:
                    sec = time.time()
                    if sec != self._sec:
                        self._sec = sec
                        self.screen.second(self.localtime())
                    to = self.screen.timeout
                    if to and time.ticks_diff(now, self.last_input) > to * 1000:
                        self.home()
                    # Optional per-loop hook for animation (screens throttle themselves)
                    frame = getattr(self.screen, 'frame', None)
                    if frame:
                        frame(now)
                self._animate(now)
            except Exception as e:
                sys.print_exception(e)
                await asyncio.sleep_ms(500)
            work = time.ticks_diff(time.ticks_us(), t0)
            t1 = time.ticks_us()
            await asyncio.sleep_ms(LOOP_MS)
            # Oversleeping means other tasks (network parsing etc.) were running
            slept = time.ticks_diff(time.ticks_us(), t1)
            self._busy_us += work + max(0, slept - LOOP_MS * 1000)
