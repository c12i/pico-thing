# A little robot for the Productivity face. draw() renders one animation frame of
# a mood into a ~100x180 box centred on cx; call it once a second with frame += 1.
from gfx import BG, DIM, MUTED, PANEL2, WHITE, ACCENT, GREEN, BLUE, YELLOW, ORANGE, RED

CAPTIONS = {
    'happy': 'You got this', 'overwhelmed': 'So much!', 'excited': 'Nice one!',
    'busy': 'Focusing', 'sleeping': 'Recharging', 'chill': 'All done!',
    'confused': 'Mac offline?', 'bored': 'Paused...',
}
BOX_W, BOX_H = 112, 178


def _caret(cv, x, y, c, up=True):
    d = -4 if up else 4
    cv.thick_line(x - 5, y, x, y + d, 3, c)
    cv.thick_line(x, y + d, x + 5, y, 3, c)


def _sparkle(cv, x, y, c):
    cv.fill_rect(x - 1, y - 5, 3, 11, c)
    cv.fill_rect(x - 5, y - 1, 11, 3, c)


def draw(cv, mood, frame, cx, top):
    """Clear the box and draw the bot. `top` is the box's top edge."""
    cv.fill_rect(cx - BOX_W // 2, top, BOX_W, BOX_H, BG)
    bounce = -4 if mood == 'excited' and frame % 2 else 0
    y0 = top + 26 + bounce                  # head top
    hx, hw, hh = cx - 32, 64, 50
    # antenna
    ball = {'busy': ACCENT if frame % 2 else ORANGE, 'excited': YELLOW,
            'overwhelmed': RED, 'sleeping': MUTED}.get(mood, GREEN)
    cv.vline(cx, y0 - 12, 12, MUTED)
    cv.ellipse(cx, y0 - 15, 4, 4, ball, True)
    # head, ears, face screen
    cv.fill_rect(hx - 5, y0 + 16, 5, 16, PANEL2)
    cv.fill_rect(hx + hw, y0 + 16, 5, 16, PANEL2)
    cv.round_rect(hx, y0, hw, hh, 12, PANEL2)
    cv.round_rect(hx + 6, y0 + 6, hw - 12, hh - 12, 8, BG)
    ey, lx, rx = y0 + 21, cx - 11, cx + 11
    my = y0 + 34
    blink = mood == 'happy' and frame % 7 == 0

    # eyes
    if mood == 'happy':
        for x in (lx, rx):
            if blink:
                cv.hline(x - 4, ey, 9, ACCENT)
            else:
                cv.ellipse(x, ey, 4, 4, ACCENT, True)
    elif mood == 'overwhelmed':
        for x in (lx, rx):
            cv.ellipse(x, ey, 5, 5, ORANGE)
            cv.ellipse(x + (1 if frame % 2 else -1), ey, 2, 2, ORANGE, True)
    elif mood == 'excited':
        _caret(cv, lx, ey + 2, YELLOW)
        _caret(cv, rx, ey + 2, YELLOW)
    elif mood == 'busy':
        for x, d in ((lx, 1), (rx, -1)):
            cv.fill_rect(x - 5, ey - 1, 11, 4, ACCENT)
            cv.thick_line(x - 6, ey - 7 - d * 2, x + 6, ey - 7 + d * 2, 2, DIM)   # determined brows
    elif mood == 'sleeping':
        _caret(cv, lx, ey - 1, DIM, up=False)
        _caret(cv, rx, ey - 1, DIM, up=False)
    elif mood == 'chill':
        cv.fill_rect(lx - 9, ey - 4, 40, 8, WHITE)       # sunglasses
        cv.fill_rect(lx - 7, ey - 6, 14, 12, WHITE)
        cv.fill_rect(rx - 7, ey - 6, 14, 12, WHITE)
    elif mood == 'confused':
        cv.ellipse(lx, ey, 4, 4, ACCENT, True)
        cv.ellipse(rx, ey + 1, 2, 2, ACCENT, True)
    else:   # bored
        for x in (lx, rx):
            cv.fill_rect(x - 5, ey, 11, 3, DIM)

    # mouth
    if mood in ('happy', 'chill'):
        if mood == 'chill':
            cv.thick_line(cx - 6, my, cx + 7, my - 3, 2, ACCENT)
        else:
            cv.ellipse(cx, my - 2, 7, 4, ACCENT, False, 4 | 8)
    elif mood == 'excited':
        cv.ellipse(cx, my - 2, 7, 6, RED, True, 4 | 8)
    elif mood == 'overwhelmed':
        pts = (cx - 8, cx - 4, cx, cx + 4, cx + 8)
        for i in range(4):
            cv.line(pts[i], my + (1 if i % 2 else -1), pts[i + 1], my + (-1 if i % 2 else 1), ORANGE)
    elif mood == 'sleeping':
        cv.ellipse(cx, my, 2, 2, DIM)
    elif mood == 'confused':
        cv.thick_line(cx - 6, my + 1, cx + 6, my - 1, 2, ACCENT)
    else:   # busy, bored
        cv.hline(cx - 5, my, 11, ACCENT if mood == 'busy' else DIM)

    # body + arms + feet
    by = y0 + hh + 6
    cv.round_rect(cx - 22, by, 44, 30, 8, PANEL2)
    cv.ellipse(cx, by + 15, 4, 4, ball, True)            # chest light matches the antenna
    if mood == 'excited':                                # arms up
        cv.thick_line(cx - 22, by + 8, cx - 36, by - 10, 5, PANEL2)
        cv.thick_line(cx + 22, by + 8, cx + 36, by - 10, 5, PANEL2)
    elif mood == 'overwhelmed':                          # hands on head
        cv.thick_line(cx - 22, by + 6, cx - 40, y0 + 22, 5, PANEL2)
        cv.thick_line(cx + 22, by + 6, cx + 40, y0 + 22, 5, PANEL2)
    else:
        cv.thick_line(cx - 22, by + 8, cx - 32, by + 26, 5, PANEL2)
        cv.thick_line(cx + 22, by + 8, cx + 32, by + 26, 5, PANEL2)
    cv.fill_rect(cx - 16, by + 30, 10, 6, MUTED)
    cv.fill_rect(cx + 6, by + 30, 10, 6, MUTED)

    # extras
    if mood == 'sleeping':
        k = frame % 3
        cv.big_text('z', cx + 26, y0 - 4 - k * 3, DIM, 1)
        if k:
            cv.big_text('Z', cx + 34, y0 - 18 - k * 2, DIM, 2)
    elif mood == 'excited':
        on = frame % 2
        _sparkle(cv, cx - 44 if on else cx + 44, y0 + 2, YELLOW)
        _sparkle(cv, cx + 40 if on else cx - 40, y0 + 40, YELLOW)
    elif mood == 'overwhelmed':
        d = (frame % 3) * 3
        cv.ellipse(hx + hw - 6, y0 + 8 + d, 3, 4, BLUE, True)     # sweat drop
    elif mood == 'confused':
        cv.big_text('?', cx + 30, y0 - 22, ORANGE, 2)

    cap = CAPTIONS.get(mood, '')
    cv.ctext(cap, cx, top + BOX_H - 12, DIM, 1)
