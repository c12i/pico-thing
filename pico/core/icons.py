# Weather icons drawn from primitives, centred on (cx, cy), roughly `size` wide.
import math
from array import array
from gfx import BG, FG, DIM, BLUE, WHITE, YELLOW


def _sun(cv, cx, cy, r):
    cv.ellipse(cx, cy, r, r, YELLOW, True)
    t = max(2, r // 4)
    for i in range(8):
        a = i * math.pi / 4
        c, s = math.cos(a), math.sin(a)
        cv.thick_line(int(cx + c * r * 1.4), int(cy + s * r * 1.4),
                      int(cx + c * r * 1.9), int(cy + s * r * 1.9), t, YELLOW)


def _moon(cv, cx, cy, r, bg):
    cv.ellipse(cx, cy, r, r, YELLOW, True)
    cv.ellipse(cx + r * 2 // 5, cy - r * 2 // 5, r * 4 // 5, r * 4 // 5, bg, True)


def _cloud(cv, cx, cy, s, c):
    """Cloud centred on (cx, cy); s is half its width."""
    p = lambda n: s * n // 100
    cv.ellipse(cx - p(52), cy + p(12), p(40), p(34), c, True)
    cv.ellipse(cx + p(2), cy - p(14), p(50), p(48), c, True)
    cv.ellipse(cx + p(56), cy + p(10), p(38), p(36), c, True)
    cv.fill_rect(cx - p(52), cy + p(12), p(108), p(34), c)


def draw(cv, kind, cx, cy, size, bg=BG):
    s = size // 2
    p = lambda n: s * n // 100
    if kind == 'sun':
        _sun(cv, cx, cy, p(48))
    elif kind == 'moon':
        _moon(cv, cx, cy, p(60), bg)
    elif kind in ('partly', 'partly_night'):
        if kind == 'partly':
            _sun(cv, cx - p(28), cy - p(28), p(32))
        else:
            _moon(cv, cx - p(28), cy - p(30), p(36), bg)
        _cloud(cv, cx + p(12), cy + p(18), p(78), FG)
    elif kind == 'cloud':
        _cloud(cv, cx + p(20), cy - p(18), p(60), DIM)
        _cloud(cv, cx - p(8), cy + p(10), p(80), FG)
    elif kind == 'fog':
        for i, w in enumerate((70, 90, 60, 80)):
            y = cy - p(45) + i * p(30)
            cv.thick_line(cx - p(w), y, cx + p(w), y, max(3, p(12)), DIM)
    else:
        # Precipitation: cloud on top, something falling below it
        _cloud(cv, cx, cy - p(25), p(85), DIM if kind == 'storm' else FG)
        top = cy + p(30)
        if kind == 'storm':
            pts = array('h', (p(12), 0, p(-18), p(38), 0, p(38), p(-12), p(72),
                              p(28), p(22), p(8), p(22), p(24), 0))
            cv.poly(cx, top - p(6), pts, YELLOW, True)
        elif kind == 'snow':
            r = max(2, p(9))
            for i, dx in enumerate((-50, 0, 50)):
                cv.ellipse(cx + p(dx), top + p(20 + (i % 2) * 22), r, r, WHITE, True)
        elif kind == 'drizzle':
            for i, dx in enumerate((-48, -16, 16, 48)):
                y = top + p(14 + (i % 2) * 20)
                cv.thick_line(cx + p(dx), y, cx + p(dx - 6), y + p(16), max(2, p(7)), BLUE)
        else:
            for dx in (-45, 0, 45):
                cv.thick_line(cx + p(dx), top + p(8), cx + p(dx - 14), top + p(50),
                              max(2, p(8)), BLUE)


# --- UI icons for the menu tiles, ~40px, drawn in `c` on background `bg` ---

def ui_icon(cv, name, cx, cy, c, bg):
    if name == 'faces':                        # clock
        cv.ellipse(cx, cy, 18, 18, c, True)
        cv.ellipse(cx, cy, 14, 14, bg, True)
        cv.thick_line(cx, cy, cx, cy - 10, 3, c)
        cv.thick_line(cx, cy, cx + 7, cy + 4, 3, c)
        cv.ellipse(cx, cy, 2, 2, c, True)
    elif name == 'lights':                     # bulb
        cv.ellipse(cx, cy - 5, 13, 13, c, True)
        cv.ellipse(cx - 4, cy - 9, 3, 3, bg, True)
        cv.fill_rect(cx - 6, cy + 6, 12, 6, c)
        cv.fill_rect(cx - 5, cy + 14, 10, 3, c)
    elif name == 'sonos':                      # speaker + sound waves
        cv.fill_rect(cx - 16, cy - 6, 8, 12, c)
        cv.poly(0, 0, array('h', (cx - 9, cy - 6, cx + 1, cy - 15, cx + 1, cy + 15, cx - 9, cy + 6)), c, True)
        for r in (8, 14):
            for k in range(3):                 # thick arc from stacked outlines
                cv.ellipse(cx + 3, cy, r + k, r + k, c, False, 9)
    elif name == 'command':                    # terminal window with a prompt
        cv.round_rect(cx - 20, cy - 15, 40, 30, 5, c)
        cv.round_rect(cx - 17, cy - 12, 34, 24, 3, bg)
        cv.thick_line(cx - 11, cy - 6, cx - 5, cy, 3, c)
        cv.thick_line(cx - 5, cy, cx - 11, cy + 6, 3, c)
        cv.fill_rect(cx - 1, cy + 5, 10, 3, c)
    elif name == 'settings':                   # gear
        for i in range(8):
            a = i * math.pi / 4
            co, si = math.cos(a), math.sin(a)
            cv.thick_line(int(cx + co * 10), int(cy + si * 10), int(cx + co * 18), int(cy + si * 18), 7, c)
        cv.ellipse(cx, cy, 13, 13, c, True)
        cv.ellipse(cx, cy, 5, 5, bg, True)
    elif name == 'home':                       # house
        cv.poly(0, 0, array('h', (cx - 20, cy - 1, cx, cy - 19, cx + 20, cy - 1)), c, True)
        cv.fill_rect(cx - 13, cy - 2, 26, 18, c)
        cv.fill_rect(cx - 4, cy + 5, 8, 11, bg)
    elif name == 'pomodoro':                   # stopwatch
        cv.fill_rect(cx - 4, cy - 22, 8, 5, c)
        cv.thick_line(cx + 13, cy - 15, cx + 17, cy - 19, 4, c)
        cv.ellipse(cx, cy + 2, 18, 18, c, True)
        cv.ellipse(cx, cy + 2, 14, 14, bg, True)
        cv.ellipse(cx, cy + 2, 14, 14, c, True, 1)     # elapsed quarter
        cv.thick_line(cx, cy + 2, cx, cy - 10, 3, bg)
