# Pomodoro timer state. Lives on the app (not a screen) so it keeps running and
# the Productivity face can show it.
import time

# (focus, short break, long break) minutes
PRESETS = ((25, 5, 15), (50, 10, 20), (15, 3, 10))
LONG_EVERY = 4
LABELS = {'focus': 'Focus', 'short': 'Short break', 'long': 'Long break'}


class Pomodoro:
    def __init__(self, settings):
        self.settings = settings
        self.preset = settings.get('pomo_preset', 0) % len(PRESETS)
        self.phase = 'focus'       # focus | short | long
        self.state = 'idle'        # idle | running | paused | done (finished, waiting for you)
        self.finished = None       # phase that just completed, while state == 'done'
        self.cycle = 0             # focus sessions completed towards the next long break
        day, n = settings.get('pomo_today', (None, 0))
        self.day, self.today = day, n
        self._end = 0
        self.left = self.duration()

    @property
    def active(self):
        return self.state != 'idle'

    @property
    def label(self):
        return LABELS[self.phase]

    def duration(self, phase=None):
        focus, short, long_ = PRESETS[self.preset]
        return {'focus': focus, 'short': short, 'long': long_}[phase or self.phase] * 60

    def remaining(self):
        if self.state == 'running':
            return max(0, time.ticks_diff(self._end, time.ticks_ms()) // 1000)
        return self.left

    def progress(self):
        return 1 - self.remaining() / self.duration()

    def start(self):
        if self.state != 'running':
            self.state, self.finished = 'running', None
            self._end = time.ticks_add(time.ticks_ms(), self.left * 1000)

    def pause(self):
        if self.state == 'running':
            self.left = self.remaining()
            self.state = 'paused'

    def reset(self):
        self.state, self.finished = 'idle', None
        self.left = self.duration()

    def skip(self):
        self._advance()
        self.state, self.finished = 'idle', None

    def next_preset(self):
        self.preset = (self.preset + 1) % len(PRESETS)
        self.settings['pomo_preset'] = self.preset
        self._save()
        if self.state in ('idle', 'done'):
            self.left = self.duration()

    def tick(self, today):
        """Call about once a second with today's day-of-month. True when a phase just ended."""
        if today != self.day:
            self.day, self.today = today, 0
        if self.state != 'running' or self.remaining() > 0:
            return False
        self.finished = self.phase
        if self.phase == 'focus':
            self.cycle += 1
            self.today += 1
            self.settings['pomo_today'] = (self.day, self.today)
            self._save()
        self._advance()
        self.state = 'done'
        return True

    def _advance(self):
        if self.phase == 'focus':
            self.phase = 'long' if self.cycle and self.cycle % LONG_EVERY == 0 else 'short'
        else:
            if self.phase == 'long':
                self.cycle = 0
            self.phase = 'focus'
        self.left = self.duration()

    def _save(self):
        import settings
        settings.save(self.settings)
