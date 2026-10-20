# Sonos now-playing, transport, volume and grouping, plus the favorites picker.
import json
import time
import config
import sonos
from gfx import W, BG, FG, DIM, MUTED, PANEL, PANEL2, ACCENT, GREEN, RED
from ui import Screen, Button

REFRESH_S = 10
OFFLINE_REFRESH_S = 30
VOL_STEP = 3


def speakers(app):
    """Speakers live on the app so their uuids/state survive leaving the screen."""
    if not hasattr(app, 'sonos'):
        app.sonos = [sonos.Speaker(name, ip) for name, ip in getattr(config, 'SONOS', ())]
    return app.sonos


def clip(s, n):
    return s if len(s) <= n else s[:n - 2] + '..'


class SonosScreen(Screen):
    feature = 'sonos'
    title = 'Sonos'
    CARD = (12, 104, W - 24, 92)
    BAR = (76, 262, 224, 50)

    def __init__(self, app):
        super().__init__(app)
        self.speakers = speakers(app)
        self.sel = getattr(app, 'sonos_sel', 0)
        self.error = ''
        self._last = 0
        self._build()

    @property
    def sp(self):
        return self.speakers[self.sel]

    def _grouped(self):
        sp = self.sp
        return sp.leader is not None or any(s.leader is sp for s in self.speakers)

    def _build(self):
        sp = self.sp
        fg = FG if sp.can_skip else MUTED      # TV/line-in: transport buttons do nothing
        self.buttons = self.buttons[:1]
        if len(self.speakers) > 1:
            self.buttons.append(Button(W - 148, 6, 136, 36, 'Ungroup' if self._grouped() else 'Group',
                                       self.toggle_group, PANEL2, FG, 2))
        n = len(self.speakers)
        tw = (W - 24 - 8 * (n - 1)) // n
        for i, s in enumerate(self.speakers):
            sel = i == self.sel
            self.buttons.append(Button(12 + i * (tw + 8), 56, tw, 40, s.name, lambda i=i: self.select(i),
                                       ACCENT if sel else PANEL, BG if sel else DIM))
        self.buttons += [
            Button(16, 204, 120, 50, '|<', lambda: self._transport(sp.previous, 'Previous track'), PANEL2, fg, 3),
            Button(144, 204, 192, 50, 'Pause' if sp.playing and sp.can_skip else 'Play', self.play_pause,
                   GREEN if sp.playing and sp.can_skip else PANEL2,
                   BG if sp.playing and sp.can_skip else fg, 3),
            Button(344, 204, 120, 50, '>|', lambda: self._transport(sp.next, 'Next track'), PANEL2, fg, 3),
            Button(16, 262, 52, 50, '-', lambda: self.set_volume(sp.volume - VOL_STEP), PANEL2, FG, 3),
            Button(308, 262, 52, 50, '+', lambda: self.set_volume(sp.volume + VOL_STEP), PANEL2, FG, 3),
            Button(368, 262, 96, 50, 'Muted' if sp.muted else 'Mute',
                   lambda: self._do(sp.set_mute(not sp.muted), 'Unmuting' if sp.muted else 'Muting'),
                   RED if sp.muted else PANEL2, BG if sp.muted else FG, 2),
        ]

    def draw(self):
        super().draw()
        cv, sp = self.cv, self.sp
        x, y, w, h = self.CARD
        cv.round_rect(x, y, w, h, 12, PANEL)
        if sp.online is False:
            cv.big_text("Can't reach " + sp.name, x + 16, y + 20, RED, 2)
            cv.big_text(sp.ip, x + 16, y + 50, DIM, 1)
        elif sp.online is None:
            cv.big_text('Connecting...', x + 16, y + 20, DIM, 2)
        else:
            title = sp.title or sp.source or ('Nothing playing' if sp.state == 'STOPPED' else '')
            cv.big_text(clip(title, 26), x + 16, y + 14, FG, 2)
            cv.big_text(clip(sp.artist, 26), x + 16, y + 40, DIM, 2)
            cv.rtext('Browse >', x + w - 14, y + 70, ACCENT, 1)
            info = {'PLAYING': 'Playing', 'PAUSED_PLAYBACK': 'Paused', 'STOPPED': 'Stopped',
                    'TRANSITIONING': 'Loading'}.get(sp.state, sp.state)
            if sp.leader:
                info += '  -  grouped with ' + sp.leader.name
            elif any(s.leader is sp for s in self.speakers):
                info += '  -  group leader'
            cv.big_text(clip(info, 40), x + 16, y + 70, ACCENT if sp.playing else MUTED, 1)
        if self.error:
            cv.ctext(clip(self.error, 56), W // 2, y + h - 4, RED, 1)
        bx, by, bw, bh = self.BAR
        cv.round_rect(bx, by, bw, bh, 10, PANEL2)
        if sp.volume:
            cv.round_rect(bx, by, max(20, bw * sp.volume // 100), bh, 10, MUTED if sp.muted else ACCENT)
        cv.ctext('%d' % sp.volume, bx + bw // 2, by + bh // 2 - 8, FG, 2)

    def enter(self):
        Screen.enter(self)
        self._last = time.time()
        self.app.task(self._refresh(self.speakers), 'Loading speakers', self._redraw)

    async def _refresh(self, which):
        for s in which:
            try:
                await s.refresh(self.speakers)
            except Exception as e:
                print('sonos refresh failed:', s.name, e)

    def _redraw(self, _res=None, err=None):
        if err:
            self.error = str(err)
        self._last = time.time()
        self._build()
        Screen.enter(self)

    def second(self, t):
        # An unreachable speaker would stall each refresh until it times out, so back off
        every = REFRESH_S if self.sp.online is not False else OFFLINE_REFRESH_S
        if time.time() - self._last >= every:
            self._last = time.time()
            self.app.background(self._refresh((self.sp,)), self._redraw)

    def select(self, i):
        self.sel = self.app.sonos_sel = i
        self.error = ''
        self._redraw()           # show cached state straight away
        self.app.task(self._refresh((self.sp,)), 'Loading ' + self.sp.name, self._redraw)

    def browse(self):
        self.app.go(FavoritesScreen(self.app, self.sp))

    def _do(self, coro, label):
        self.error = ''
        self.app.task(coro, label, self._redraw)

    def _transport(self, fn, label):
        if not self.sp.can_skip:
            self.error = "%s audio can't be paused or skipped" % self.sp.source
            self._redraw()
            return
        self._do(fn(), label)

    def set_volume(self, v):
        v = min(max(v, 0), 100)
        self._do(self.sp.set_volume(v), 'Volume %d' % v)

    def play_pause(self):
        sp = self.sp
        if not sp.can_skip:
            self._transport(None, '')
            return
        if not sp.playing and sp.state == 'STOPPED' and not (sp.title or sp.source):
            self.browse()       # nothing queued: pick something instead of failing
            return
        if sp.playing:
            self._do(sp.pause(), 'Pausing')
        else:
            self._do(sp.play(), 'Playing')

    def toggle_group(self):
        sp = self.sp
        if sp.leader:
            self._do(sp.leave(), 'Leaving group')
        elif any(s.leader is sp for s in self.speakers):
            async def ungroup():
                for s in self.speakers:
                    if s.leader is sp:
                        await s.leave()
            self._do(ungroup(), 'Ungrouping')
        else:
            # Pull every other speaker into this one's music
            async def group():
                for s in self.speakers:
                    if s is not sp:
                        if not s.uuid:
                            await s.load_uuid()
                        if not sp.uuid:
                            await sp.load_uuid()
                        await s.join(sp)
            self._do(group(), 'Grouping')

    def tap(self, x, y):
        bx, by, bw, bh = self.BAR
        if bx <= x < bx + bw and by <= y < by + bh:
            self.set_volume((x - bx) * 100 // bw)
            return
        cx, cy, cw, ch = self.CARD
        if cx <= x < cx + cw and cy <= y < cy + ch:
            self.browse()
            return
        super().tap(x, y)


class FavoritesScreen(Screen):
    """Sonos Favorites, split into stations/playlists ("Mixes") and single tracks.
    The list is cached in flash since fetching it takes several seconds."""
    feature = 'sonos'
    ROWS = 5
    CACHE = 'favorites.json'

    def __init__(self, app, speaker):
        self.title = speaker.name
        super().__init__(app)
        self.sp = speaker
        self.tab = 0
        self.page = 0
        self.error = ''
        if getattr(app, 'sonos_favs', None) is None:
            app.sonos_favs = self._load_cache()
        self._build()

    def back(self):
        self.app.go(SonosScreen(self.app))

    def _load_cache(self):
        try:
            with open(self.CACHE) as f:
                return [tuple(x) for x in json.load(f)]
        except (OSError, ValueError):
            return None

    def _items(self):
        mixes = self.tab == 0
        return [f for f in self.app.sonos_favs or () if f[2] == mixes]

    def _build(self):
        items = self._items()
        pages = max(1, (len(items) + self.ROWS - 1) // self.ROWS)
        self.page = min(self.page, pages - 1)
        self.pages = pages
        self.buttons = self.buttons[:1]
        self.buttons += [
            Button(W - 128, 6, 56, 36, '^', lambda: self.turn(-1), PANEL2, FG if self.page else MUTED, 2),
            Button(W - 66, 6, 56, 36, 'v', lambda: self.turn(1), PANEL2, FG if self.page < pages - 1 else MUTED, 2),
        ]
        for i, label in enumerate(('Mixes', 'Tracks')):
            sel = i == self.tab
            self.buttons.append(Button(12 + i * 168, 54, 160, 36, label, lambda i=i: self.switch(i),
                                       ACCENT if sel else PANEL, BG if sel else DIM))
        self.buttons.append(Button(348, 54, 120, 36, 'Reload', self.reload, PANEL, DIM))
        start = self.page * self.ROWS
        for k, (index, title, _) in enumerate(items[start:start + self.ROWS]):
            self.buttons.append(Button(12, 98 + k * 44, W - 24, 38, clip(title, 27),
                                       lambda i=index, t=title: self.play(i, t), PANEL2, FG, 2))

    def draw(self):
        super().draw()
        cv = self.cv
        cv.rtext('%d/%d' % (self.page + 1, self.pages), W - 140, 20, DIM, 1)
        if self.app.sonos_favs is not None and not self._items():
            cv.ctext('No favorites here', W // 2, 180, DIM, 2)
        if self.error:
            cv.ctext(clip(self.error, 56), W // 2, 308, RED, 1)

    def _redraw(self, _res=None, err=None):
        if err:
            self.error = str(err)
        self._build()
        Screen.enter(self)

    def enter(self):
        Screen.enter(self)
        if self.app.sonos_favs is None:
            self.reload()

    def reload(self):
        self.error = ''
        self.app.task(self._reload(), 'Loading favorites', self._redraw)

    async def _reload(self):
        app = self.app
        favs = await self.sp.favorites(
            lambda done, total: app.set_busy_label('Favorites %d/%d' % (done, total)))
        app.sonos_favs = favs
        with open(self.CACHE, 'w') as f:
            json.dump(favs, f)
        self.page = 0

    def turn(self, d):
        if 0 <= self.page + d < self.pages:
            self.page += d
            self._redraw()

    def switch(self, tab):
        self.tab, self.page = tab, 0
        self._redraw()

    def play(self, index, title):
        self.error = ''
        self.app.task(self._play(index, title), 'Starting ' + title, self._played)

    async def _play(self, index, title):
        t = self.sp.target
        if not t.uuid:
            await t.load_uuid()
        if await self.sp.play_favorite(index, title):
            return True
        # Favorites changed since the list was cached: refresh it instead
        await self._reload()
        return False

    def _played(self, ok, err):
        if err:
            self._redraw(None, err)
        elif not ok:
            self.error = 'Favorites changed - list reloaded, pick again'
            self._redraw()
        else:
            self.back()
