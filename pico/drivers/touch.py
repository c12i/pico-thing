# XPT2046 resistive touch controller, sharing SPI1 with the LCD.
from machine import Pin
from lcd import spi_baud

TP_CS = 16
TP_IRQ = 17
TOUCH_HZ = 2_000_000
MIN_PRESSURE = 80     # z1 below this is a graze; positions are garbage
MAX_SPREAD = 200      # samples disagreeing more than this mean the finger moved/lifted

# Least-squares fit from ~60 taps on this panel (raw y -> screen x, raw x -> screen y)
DEFAULT_CALIB = {'swap': True, 'ax': 0.146, 'bx': -54.27, 'ay': -0.10254, 'by': 359.84}


class Touch:
    def __init__(self, spi, lcd_baud, calib=None):
        self.spi = spi
        self.lcd_baud = lcd_baud
        self.touch_baud = spi_baud(TOUCH_HZ)
        self.cs = Pin(TP_CS, Pin.OUT, value=1)
        self.irq = Pin(TP_IRQ, Pin.IN, Pin.PULL_UP)   # low while pressed
        self.calib = calib or DEFAULT_CALIB
        self._out = bytearray(3)
        self._in = bytearray(3)

    def _sample(self, cmd):
        self._out[0] = cmd
        self.cs(0)
        self.spi.write_readinto(self._out, self._in)
        self.cs(1)
        return ((self._in[1] << 8) | self._in[2]) >> 3

    def raw(self):
        """Median of a few 12-bit (x, y) samples, or None if not pressed."""
        if self.irq():
            return None
        self.spi.init(baudrate=self.touch_baud)
        xs, ys = [], []
        for _ in range(7):
            xs.append(self._sample(0xD0))
            ys.append(self._sample(0x90))
        z1 = self._sample(0xB0)
        self.spi.init(baudrate=self.lcd_baud)
        if self.irq() or z1 < MIN_PRESSURE:
            return None
        xs.sort()
        ys.sort()
        # Ignore the extremes, then insist the rest agree
        if xs[5] - xs[1] > MAX_SPREAD or ys[5] - ys[1] > MAX_SPREAD:
            return None
        x, y = xs[3], ys[3]
        # Real touches on this panel stay well inside ~500..3900; 0/4095 is a floating read
        if not (150 < x < 3950 and 150 < y < 3950):
            return None
        return x, y

    def map(self, r):
        """Raw reading -> screen (x, y) using the stored calibration."""
        c = self.calib
        if not c:
            return None
        if 'fx' in c:
            x, y = _affine(c, r)
        else:
            u, v = (r[1], r[0]) if c['swap'] else r
            x, y = c['ax'] * u + c['bx'], c['ay'] * v + c['by']
        return min(max(int(x), 0), 479), min(max(int(y), 0), 319)


# --- affine calibration: screen = a*u' + b*v' + c, with raw (u, v) centred and
# scaled to ~±2 first so single-precision floats keep their accuracy ---

def _affine(c, r):
    u, v = (r[0] - c['mu']) / 1000, (r[1] - c['mv']) / 1000
    fx, fy = c['fx'], c['fy']
    return fx[0] * u + fx[1] * v + fx[2], fy[0] * u + fy[1] * v + fy[2]


def _solve3(m, rhs):
    """Cramer's rule for a 3x3 system."""
    def det(a):
        return (a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
                - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
                + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]))
    d = det(m) or 1e-9
    out = []
    for col in range(3):
        mc = [[rhs[i] if j == col else m[i][j] for j in range(3)] for i in range(3)]
        out.append(det(mc) / d)
    return out


def fit_affine(raws, targets):
    """Least-squares fit of raw readings to screen targets -> (calib, mean error px)."""
    n = len(raws)
    mu = sum(r[0] for r in raws) / n
    mv = sum(r[1] for r in raws) / n
    pts = [((r[0] - mu) / 1000, (r[1] - mv) / 1000) for r in raws]
    suu = sum(u * u for u, _ in pts)
    suv = sum(u * v for u, v in pts)
    svv = sum(v * v for _, v in pts)
    su = sum(u for u, _ in pts)
    sv = sum(v for _, v in pts)
    m = ((suu, suv, su), (suv, svv, sv), (su, sv, n))
    fits = []
    for axis in (0, 1):
        rhs = (sum(u * t[axis] for (u, _), t in zip(pts, targets)),
               sum(v * t[axis] for (_, v), t in zip(pts, targets)),
               sum(t[axis] for t in targets))
        fits.append(_solve3(m, rhs))
    cal = {'mu': mu, 'mv': mv, 'fx': fits[0], 'fy': fits[1]}
    err = 0
    for r, (tx, ty) in zip(raws, targets):
        x, y = _affine(cal, r)
        err += ((x - tx) ** 2 + (y - ty) ** 2) ** 0.5
    return cal, err / n
