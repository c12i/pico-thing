# Home screen faces: shared helpers, the Face base classes and the registry.
# Each face lives in its own face_<key>.py; only the one on screen stays loaded.
import sys
from gfx import W, BG, FG, DIM, MUTED, ACCENT, GREEN, ORANGE
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


# (settings key, name in the picker, class in face_<key>.py)
FACES = (
    ('digital', 'Digital', 'DigitalFace'),
    ('analog', 'Analog', 'AnalogFace'),
    ('weather', 'Weather', 'WeatherFace'),
    ('productivity', 'Productivity', 'ProductivityFace'),
    ('bonsai', 'Bonsai', 'BonsaiFace'),
)


def make(app, key):
    key = {'focus': 'productivity'}.get(key, key)     # renamed
    entry = FACES[0]
    for f in FACES:
        if f[0] == key:
            entry = f
        else:
            sys.modules.pop('face_' + f[0], None)    # unload faces that aren't showing
    return getattr(__import__('face_' + entry[0]), entry[2])(app)
