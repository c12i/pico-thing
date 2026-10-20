# A bonsai that keeps growing (see bonsai.py) under the top strip.
import framebuf
import time
import bonsai
from gfx import W, H, BG, DIM, GREEN, GREEN2, YELLOW, ORANGE
from faces import StripFace


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
        self._glyph = framebuf.FrameBuffer(bytearray(8), 8, 8, framebuf.MONO_HLSB)
        self._t = 0
        self._done = None

    def draw(self):
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
