# Full-screen 16-colour canvas plus the drawing helpers the UI needs.
# Drawing happens in RAM (GS4, 77KB); flush() pushes a region to the panel.
import framebuf
import math
from array import array
from lcd import rgb, WIDTH as W, HEIGHT as H

# Palette slots
(BG, PANEL, PANEL2, MUTED, DIM, FG, WHITE, ACCENT, ACCENT2,
 RED, GREEN, BLUE, YELLOW, ORANGE, PURPLE, GREEN2) = range(16)


def _hex(h):
    return rgb(int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


# Gruvbox (https://github.com/morhetz/gruvbox), one colour per palette slot
THEMES = {
    'dark': tuple(_hex(h) for h in (
        '282828',   # BG       bg0
        '3c3836',   # PANEL    bg1
        '504945',   # PANEL2   bg2
        '665c54',   # MUTED    bg3
        'a89984',   # DIM      fg4
        'ebdbb2',   # FG       fg
        'fbf1c7',   # WHITE    fg0 (highest contrast)
        '83a598',   # ACCENT   blue
        'fe8019',   # ACCENT2  orange
        'fb4934',   # RED
        'b8bb26',   # GREEN
        '458588',   # BLUE
        'fabd2f',   # YELLOW
        'd65d0e',   # ORANGE
        'd3869b',   # PURPLE
        '98971a',   # GREEN2   neutral green (bonsai leaves)
    )),
    'light': tuple(_hex(h) for h in (
        'fbf1c7',   # BG       bg0
        'ebdbb2',   # PANEL    bg1
        'd5c4a1',   # PANEL2   bg2
        'bdae93',   # MUTED    bg3
        '7c6f64',   # DIM      fg4
        '3c3836',   # FG       fg
        '282828',   # WHITE    fg0 (highest contrast)
        '076678',   # ACCENT   blue
        'af3a03',   # ACCENT2  orange
        '9d0006',   # RED
        '79740e',   # GREEN
        '458588',   # BLUE
        'b57614',   # YELLOW
        'd65d0e',   # ORANGE
        '8f3f71',   # PURPLE
        '98971a',   # GREEN2
    )),
}
THEME = THEMES['dark']

# Seven-segment bitmasks (a..g) for 0-9, then 10 = dash
_SEG = (0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F, 0x40)
DASH = 10
SCRATCH = 280 * 60 // 2


def wrap(text, width):
    """Word-wrap to lines of at most `width` characters (long words are split)."""
    lines = []
    for para in text.replace('\r', '').split('\n'):
        line = ''
        for word in para.strip().split(' '):
            while len(word) > width:
                if line:
                    lines.append(line)
                    line = ''
                lines.append(word[:width])
                word = word[width:]
            if not line:
                line = word
            elif len(line) + 1 + len(word) <= width:
                line += ' ' + word
            else:
                lines.append(line)
                line = word
        lines.append(line)
    while lines and not lines[-1]:
        lines.pop()
    return lines


class Canvas(framebuf.FrameBuffer):
    def __init__(self, display):
        buf = bytearray(W * H // 2)
        super().__init__(buf, W, H, framebuf.GS4_HMSB)
        self.buf = buf
        self.lcd = display
        self.pal = bytearray(32)
        self.lut = bytearray(1024)
        # Scratch for temporarily drawing over the canvas (the loading box); allocated
        # here, next to the framebuffer, before the heap gets fragmented
        self.scratch = bytearray(SCRATCH)
        self.set_palette(THEME)

    def set_palette(self, colors, start=0):
        p = self.pal
        for i, c in enumerate(colors):
            p[2 * (start + i)] = c >> 8
            p[2 * (start + i) + 1] = c & 0xFF
        # Byte -> its two pixels' output bytes, so flushing is one lookup per byte
        lut = self.lut
        for b in range(256):
            hi, lo, k = (b >> 4) * 2, (b & 15) * 2, b * 4
            lut[k], lut[k + 1], lut[k + 2], lut[k + 3] = p[hi], p[hi + 1], p[lo], p[lo + 1]

    def flush(self, x=0, y=0, w=W, h=H):
        self.lcd.blit_gs4(self.buf, self.lut, x, y, w, h)

    def color565(self, slot):
        return (self.pal[2 * slot] << 8) | self.pal[2 * slot + 1]

    def save(self, x, y, w, h, store):
        """Copy a region's raw pixels into `store` (x and w even; len >= w*h/2)."""
        self._copy(x, y, w, h, store, True)

    def restore(self, x, y, w, h, store):
        self._copy(x, y, w, h, store, False)

    def _copy(self, x, y, w, h, store, out):
        rb = w // 2
        fb, st = memoryview(self.buf), memoryview(store)
        for r in range(h):
            o = ((y + r) * W + x) >> 1
            if out:
                st[r * rb:(r + 1) * rb] = fb[o:o + rb]
            else:
                fb[o:o + rb] = st[r * rb:(r + 1) * rb]

    # --- text (built-in 8x8 font, integer scaling) ---

    def big_text(self, s, x, y, c, scale=1):
        if scale == 1:
            self.text(s, x, y, c)
            return
        w = 8 * len(s)
        if not w:
            return
        m = framebuf.FrameBuffer(bytearray(w), w, 8, framebuf.MONO_HLSB)
        m.text(s, 0, 0, 1)
        px = m.pixel
        fr = self.fill_rect
        for row in range(8):
            yy = y + row * scale
            col = 0
            while col < w:
                if px(col, row):
                    start = col
                    while col < w and px(col, row):
                        col += 1
                    fr(x + start * scale, yy, (col - start) * scale, scale, c)
                else:
                    col += 1

    def ctext(self, s, cx, y, c, scale=1):
        self.big_text(s, cx - 4 * len(s) * scale, y, c, scale)

    def rtext(self, s, rx, y, c, scale=1):
        self.big_text(s, rx - 8 * len(s) * scale, y, c, scale)

    def deg_width(self, s, scale):
        return 8 * len(s) * scale + 4 * scale

    def deg_text(self, s, x, y, c, scale):
        """Text followed by a degree ring (the font has no degree sign)."""
        self.big_text(s, x, y, c, scale)
        r = max(2, scale * 3 // 2)
        rx = x + 8 * len(s) * scale + scale + r
        for i in range(max(1, scale // 2)):
            self.ellipse(rx, y + r, r - i, r - i, c)

    # --- shapes ---

    def round_rect(self, x, y, w, h, r, c):
        self.fill_rect(x + r, y, w - 2 * r, h, c)
        self.fill_rect(x, y + r, r, h - 2 * r, c)
        self.fill_rect(x + w - r, y + r, r, h - 2 * r, c)
        self.ellipse(x + r, y + r, r, r, c, True, 2)
        self.ellipse(x + w - r - 1, y + r, r, r, c, True, 1)
        self.ellipse(x + r, y + h - r - 1, r, r, c, True, 4)
        self.ellipse(x + w - r - 1, y + h - r - 1, r, r, c, True, 8)

    def thick_line(self, x0, y0, x1, y1, t, c):
        dx, dy = x1 - x0, y1 - y0
        n = math.sqrt(dx * dx + dy * dy) or 1
        nx, ny = -dy / n * t / 2, dx / n * t / 2
        pts = array('h', (int(x0 + nx), int(y0 + ny), int(x1 + nx), int(y1 + ny),
                          int(x1 - nx), int(y1 - ny), int(x0 - nx), int(y0 - ny)))
        self.poly(0, 0, pts, c, True)

    def digit(self, d, x, y, w, h, t, c):
        """Seven-segment digit; d in 0-9 or DASH."""
        bits = _SEG[d]
        hh = (h - 3 * t) // 2
        g = max(1, t // 4)
        hw = w - 2 * t - 2 * g
        vh = hh - 2 * g
        segs = (
            (x + t + g, y, hw, t),                       # a
            (x + w - t, y + t + g, t, vh),               # b
            (x + w - t, y + 2 * t + hh + g, t, vh),      # c
            (x + t + g, y + 2 * t + 2 * hh, hw, t),      # d
            (x, y + 2 * t + hh + g, t, vh),              # e
            (x, y + t + g, t, vh),                       # f
            (x + t + g, y + t + hh, hw, t),              # g
        )
        for i in range(7):
            if bits & (1 << i):
                sx, sy, sw, sh = segs[i]
                self.fill_rect(sx, sy, sw, sh, c)
