# Today's weather in detail, with a small clock on top.
import icons
from gfx import W, BG, FG, DIM, PANEL
from faces import Face, deg, date_str


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
