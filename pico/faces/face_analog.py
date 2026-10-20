# Analog clock on the left; date and weather on the right.
import math
import icons
from gfx import H, BG, FG, DIM, MUTED, PANEL, ACCENT2
from faces import Face, DAYS, MONTHS, deg


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
