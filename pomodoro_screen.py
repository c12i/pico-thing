# Pomodoro timer screen: progress ring, countdown and controls.
import math
import pomodoro
from gfx import W, H, BG, FG, DIM, MUTED, PANEL, PANEL2, ACCENT, GREEN, RED, ORANGE
from ui import Screen, Button, HEADER

CX, CY, R = 152, 186, 118
TICKS = 60


def phase_color(p):
    return ACCENT if p.phase == 'focus' else GREEN


class PomodoroScreen(Screen):
    title = 'Pomodoro'
    feature = 'pomodoro'

    def __init__(self, app, alert=False):
        super().__init__(app)
        self.p = app.pomo
        self.alert = alert
        self.timeout = 90 if alert else 30
        self._trig = [(math.sin(i * 2 * math.pi / TICKS), -math.cos(i * 2 * math.pi / TICKS))
                      for i in range(TICKS)]
        self._build()

    def back(self):
        self.app.home()

    def _build(self):
        p = self.p
        main = {'idle': 'Start', 'running': 'Pause', 'paused': 'Resume'}.get(p.state)
        if p.state == 'done':
            main = 'Start break' if p.phase != 'focus' else 'Start focus'
        running = p.state == 'running'
        focus, short, _ = pomodoro.PRESETS[p.preset]
        self.buttons = self.buttons[:1] + [
            Button(304, 60, 164, 56, main, self.main_action,
                   PANEL2 if running else phase_color(p), FG if running else BG, 2),
            Button(304, 124, 164, 48, 'Skip', self.skip),
            Button(304, 180, 164, 48, 'Reset', self.reset),
            Button(304, 236, 164, 44, '%d / %d min' % (focus, short), self.preset, PANEL, DIM, 2),
        ]

    def draw(self):
        super().draw()
        cv, p = self.cv, self.p
        cv.rtext('Today: %d' % p.today, W - 76, 20, DIM, 1)
        self._draw_ring()
        # Sessions towards the long break
        for i in range(pomodoro.LONG_EVERY):
            filled = i < p.cycle % pomodoro.LONG_EVERY or (p.cycle and p.cycle % pomodoro.LONG_EVERY == 0
                                                           and p.phase == 'long')
            x = 304 + 20 + i * 40
            cv.ellipse(x, 300, 8, 8, ORANGE if filled else PANEL2, True)

    def _draw_ring(self):
        cv, p = self.cv, self.p
        cv.fill_rect(0, HEADER, 300, H - HEADER, BG)
        done = int(p.progress() * TICKS + 0.5) if p.active else 0
        color = phase_color(p)
        for i, (s, c) in enumerate(self._trig):
            cv.thick_line(int(CX + s * (R - 16)), int(CY + c * (R - 16)),
                          int(CX + s * R), int(CY + c * R), 5, color if i < done else PANEL2)
        left = p.remaining()
        cv.ctext('%02d:%02d' % (left // 60, left % 60), CX, CY - 26, FG, 4)
        if p.state == 'done':
            msg, col = ('Focus done!' if p.finished == 'focus' else 'Break over'), ORANGE
        elif p.state == 'paused':
            msg, col = 'Paused', DIM
        else:
            msg, col = p.label, color
        cv.ctext(msg, CX, CY + 18, col, 2)
        if p.state == 'done':
            cv.ctext('Next: ' + p.label, CX, CY + 44, DIM, 1)

    def second(self, t):
        if self.p.state == 'running':
            self._draw_ring()
            self.cv.flush(0, HEADER, 300, H - HEADER)

    def _refresh(self):
        self._build()
        self.enter()

    def main_action(self):
        p = self.p
        if p.state == 'running':
            p.pause()
        else:
            p.start()
        self.alert = False
        self._refresh()

    def skip(self):
        self.p.skip()
        self._refresh()

    def reset(self):
        self.p.reset()
        self._refresh()

    def preset(self):
        if self.p.state == 'running':
            return                      # don't change the length mid-session
        self.p.next_preset()
        self._refresh()
