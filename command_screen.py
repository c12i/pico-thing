# Command center: Claude Code sessions, saved agent tasks and Mac actions, all
# through the companion server on the Mac (companion/server.py).
import companion
from gfx import wrap, W, BG, FG, DIM, MUTED, PANEL, PANEL2, ACCENT, ORANGE, GREEN, RED
from ui import Screen, Button, HEADER

STATE_COLORS = {'waiting': ORANGE, 'working': ACCENT, 'done': GREEN, 'idle': MUTED,
                'running': ACCENT, 'failed': RED}
STATE_LABELS = {'waiting': 'Needs you', 'working': 'Working', 'done': 'Finished', 'idle': 'Idle',
                'running': 'Running', 'failed': 'Failed'}
TABS = ('Agents', 'Tasks', 'Mac')
ROW_Y, ROW_H, ROW_GAP = 98, 48, 6


def clip(s, n):
    return s if len(s) <= n else s[:n - 2] + '..'


class Row(Button):
    """Title + coloured state on top, detail text underneath."""
    def __init__(self, y, title, state, detail, action):
        super().__init__(12, y, W - 24, ROW_H, title, action, PANEL)
        self.state, self.detail = state, detail

    def draw(self, cv, pressed=False):
        cv.round_rect(self.x, self.y, self.w, self.h, 10, PANEL2 if pressed else PANEL)
        color = STATE_COLORS.get(self.state, MUTED)
        cv.fill_rect(self.x, self.y + 8, 4, self.h - 16, color)
        label = STATE_LABELS.get(self.state, self.state)
        cv.big_text(clip(self.label, 26 - len(label) // 2), self.x + 14, self.y + 8, FG, 2)
        cv.rtext(label, self.x + self.w - 12, self.y + 10, color, 1)
        cv.big_text(clip(self.detail, 54), self.x + 14, self.y + 32, DIM, 1)


class CommandScreen(Screen):
    title = 'Command'
    feature = 'command'

    def __init__(self, app, tab=0):
        super().__init__(app)
        self.tab = tab
        self.error = ''
        self._build()

    @property
    def st(self):
        return getattr(self.app, 'companion', None) or {}

    def _build(self):
        self.buttons = self.buttons[:1]
        tw = (W - 24 - 16) // 3
        for i, name in enumerate(TABS):
            sel = i == self.tab
            self.buttons.append(Button(12 + i * (tw + 8), 54, tw, 36, name, lambda i=i: self.switch(i),
                                       ACCENT if sel else PANEL, BG if sel else DIM))
        if self.tab == 0:
            for k, s in enumerate(self.st.get('sessions', [])[:4]):
                self.buttons.append(Row(ROW_Y + k * (ROW_H + ROW_GAP), s['project'], s['state'],
                                        s['message'] or 'Tap to open its terminal',
                                        lambda sid=s['id']: self.focus(sid)))
        elif self.tab == 1:
            for k, t in enumerate(self.st.get('tasks', [])[:4]):
                hint = {'idle': 'Tap to run', 'running': 'Tap to watch',
                        'done': 'Tap to read the result', 'failed': 'Tap to see what went wrong'}
                self.buttons.append(Row(ROW_Y + k * (ROW_H + ROW_GAP), t['name'], t['state'],
                                        hint.get(t['state'], ''), lambda i=k, st=t['state']: self.open_task(i, st)))
        else:
            bw = (W - 24 - 8) // 2
            for k, a in enumerate(self.st.get('actions', [])[:6]):
                on = a.get('on')
                self.buttons.append(Button(12 + (k % 2) * (bw + 8), ROW_Y + (k // 2) * 60, bw, 52,
                                           clip(a['label'], 13), lambda a=a: self.action(a),
                                           ACCENT if on else PANEL2, BG if on else FG))

    def draw(self):
        super().draw()
        cv = self.cv
        empty = ('No Claude Code sessions yet', 'No tasks in companion/config.json',
                 'No actions configured')[self.tab]
        if not self.st:
            cv.ctext("Can't reach your Mac" if self.error else 'Connecting...', W // 2, 180, DIM, 2)
        elif len(self.buttons) <= 4:
            cv.ctext(empty, W // 2, 180, DIM, 2)
        if self.error:
            cv.ctext(clip(self.error, 56), W // 2, 308, RED, 1)

    def _redraw(self, _res=None, err=None):
        self.error = str(err) if err else ''
        self._build()
        Screen.enter(self)

    def enter(self):
        Screen.enter(self)
        self.app.task(self._refresh(), 'Asking your Mac', self._redraw)

    async def _refresh(self):
        self.app.companion = await companion.status()

    def on_companion(self):
        """Fresh status arrived from a background poll."""
        self._redraw()

    def switch(self, tab):
        self.tab = tab
        self._redraw()

    def focus(self, sid):
        async def go():
            await companion.call('POST', '/focus?id=' + sid)
            await self._refresh()
        self.app.task(go(), 'Opening terminal', self._redraw)

    def open_task(self, i, state):
        if state == 'idle':
            async def run():
                await companion.call('POST', '/run?n=%d' % i)
                await self._refresh()
            self.app.task(run(), 'Starting task', lambda r, e: self._redraw(r, e) if e else
                          self.app.go(TaskResult(self.app, i)))
        else:
            self.app.go(TaskResult(self.app, i))

    def action(self, a):
        async def go():
            await companion.call('POST', '/action?id=' + a['id'])
            await self._refresh()
        self.app.task(go(), a['label'], self._redraw)


class TaskResult(Screen):
    """A task's output, paged. Polls while the task is still running."""
    title = 'Result'
    feature = 'command'
    LINES, WIDTH = 11, 28
    POLL_S = 5

    def __init__(self, app, index):
        super().__init__(app)
        self.index = index
        self.data = None
        self.lines = []
        self.page = 0
        self.error = ''
        self._wait = 0
        self._build()

    def back(self):
        self.app.go(CommandScreen(self.app, tab=1))

    @property
    def pages(self):
        return max(1, (len(self.lines) + self.LINES - 1) // self.LINES)

    def _build(self):
        self.buttons = self.buttons[:1]
        more_up, more_down = self.page > 0, self.page < self.pages - 1
        self.buttons += [
            Button(W - 196, 6, 64, 36, 'Run', self.rerun, PANEL2, FG, 2),
            Button(W - 124, 6, 56, 36, '^', lambda: self.turn(-1), PANEL2, FG if more_up else MUTED, 2),
            Button(W - 62, 6, 56, 36, 'v', lambda: self.turn(1), PANEL2, FG if more_down else MUTED, 2),
        ]

    def draw(self):
        super().draw()
        cv = self.cv
        d = self.data or {}
        state = d.get('state', '')
        cv.big_text(clip(d.get('name', ''), 40), 16, 56, DIM, 1)
        cv.rtext(STATE_LABELS.get(state, state), W - 16, 56, STATE_COLORS.get(state, MUTED), 1)
        if state == 'running':
            cv.ctext('Claude is working on it...', W // 2, 170, DIM, 2)
        start = self.page * self.LINES
        for k, line in enumerate(self.lines[start:start + self.LINES]):
            cv.big_text(line, 16, 74 + k * 22, FG, 2)
        if self.error:
            cv.ctext(clip(self.error, 56), W // 2, 308, RED, 1)

    def _show(self, _res=None, err=None):
        self.error = str(err) if err else ''
        d = self.data or {}
        self.lines = wrap(d.get('output', ''), self.WIDTH) if d.get('state') != 'running' else []
        self.page = min(self.page, self.pages - 1)
        self._build()
        Screen.enter(self)

    async def _fetch(self):
        self.data = await companion.call('GET', '/task?n=%d' % self.index, timeout=6)

    def enter(self):
        Screen.enter(self)
        self.app.task(self._fetch(), 'Loading result', self._show)

    def second(self, t):
        if (self.data or {}).get('state') == 'running':
            self._wait += 1
            if self._wait >= self.POLL_S:
                self._wait = 0
                self.app.background(self._fetch(), self._show)

    def on_companion(self):
        # The companion pokes us when a task finishes
        if (self.data or {}).get('state') == 'running':
            self.app.background(self._fetch(), self._show)

    def turn(self, d):
        if 0 <= self.page + d < self.pages:
            self.page += d
            self._show()

    def rerun(self):
        async def go():
            await companion.call('POST', '/run?n=%d' % self.index)
            await self._fetch()
        self.page = 0
        self.app.task(go(), 'Starting task', self._show)


class AttentionScreen(Screen):
    """Takes over the home face when a Claude Code session needs you (or finished)."""
    feature = 'command'
    timeout = None
    AUTO_DISMISS_S = 15     # untouched alerts close themselves (and count as seen)

    def __init__(self, app, sid):
        super().__init__(app)
        self.sid = sid
        self._secs = 0
        self._state = None
        self.buttons = [
            Button(16, 252, 288, 56, 'Focus terminal', self.focus, ACCENT, BG, 2),
            Button(316, 252, 148, 56, 'Dismiss', self.dismiss, PANEL2, FG, 2),
        ]

    @property
    def session(self):
        for s in (getattr(self.app, 'companion', None) or {}).get('sessions', ()):
            if s['id'] == self.sid:
                return s
        return None

    def draw(self):
        super().draw()
        cv, s = self.cv, self.session or {}
        waiting = s.get('state') == 'waiting'
        color = ORANGE if waiting else GREEN
        cv.fill_rect(0, 0, W, 52, color)
        cv.ctext('Claude needs you' if waiting else 'Claude finished', W // 2, 14, BG, 3)
        cv.ctext(clip(s.get('project', ''), 18), W // 2, 76, FG, 3)
        for k, line in enumerate(wrap(s.get('message', ''), 28)[:4]):
            cv.ctext(line, W // 2, 124 + k * 26, DIM, 2)
        self._draw_countdown()

    def _draw_countdown(self):
        cv = self.cv
        cv.fill_rect(0, 230, W, 14, BG)
        cv.ctext('Closing in %ds' % max(0, self.AUTO_DISMISS_S - self._secs), W // 2, 232, MUTED, 1)

    def second(self, t):
        s = self.session
        if not s or s['state'] not in ('waiting', 'done'):
            self.app.home()           # handled at the terminal already
            return
        self._secs += 1
        if self._secs >= self.AUTO_DISMISS_S:
            # Mark it seen in the background so it doesn't pop up again; no loading box
            self.app.background(companion.call('POST', '/dismiss?id=' + self.sid))
            self.app.home()
            return
        self._draw_countdown()
        self.cv.flush(0, 230, W, 14)

    def on_companion(self):
        s = self.session
        if s and s['state'] in ('waiting', 'done'):
            if s['state'] != self._state:     # a new event: give it the full 15s again
                self._state, self._secs = s['state'], 0
            self.enter()

    def _post(self, path, label):
        async def go():
            await companion.call('POST', path + '?id=' + self.sid)
        self.app.task(go(), label, lambda r, e: self.app.home())

    def focus(self):
        self._post('/focus', 'Opening terminal')

    def dismiss(self):
        self._post('/dismiss', 'Dismissing')
