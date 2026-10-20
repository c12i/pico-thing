# Big seven-segment clock with the weather underneath.
import icons
from gfx import W, BG, FG, DIM, MUTED, PANEL, ACCENT, DASH
from faces import Face, deg, date_str


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
