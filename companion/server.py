#!/usr/bin/env python3
"""pico-thing companion: lets the Pico see Claude Code sessions, run saved agent
tasks, and trigger a fixed list of Mac actions. Standard library only.

    python3 companion/server.py          # serve (config.json is created on first run)

Every request needs the X-Token header from config.json. The Pico can only trigger
actions and tasks defined in config.json; it never sends commands to run.
"""
import json
import os
import re
import secrets
import shutil
import subprocess
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

HERE = Path(__file__).resolve().parent
CONFIG_PATH = HERE / 'config.json'

DEFAULT_CONFIG = {
    'port': 8765,
    'pico_port': 8080,
    'focus_minutes': 25,
    # Extra buttons for the Mac tab. "cmd" is an argv list, run without a shell.
    'actions': [
        {'id': 'slack', 'label': 'Open Slack', 'cmd': ['open', '-a', 'Slack']},
        # {'id': 'dnd', 'label': 'Focus mode', 'cmd': ['shortcuts', 'run', 'Toggle Focus']},
    ],
    # Saved prompts run as `claude -p` in `cwd`, limited to `allowed_tools`
    'tasks': [
        {'name': 'Project status', 'cwd': str(HERE.parent),
         'prompt': 'Summarize the current state of this project in 5 short bullet points. '
                   'Plain text, no markdown, under 600 characters.',
         'allowed_tools': 'Read Glob Grep'},
        {'name': "What's next", 'cwd': str(HERE.parent),
         'prompt': 'Read TODO.md and list the next 3 things to do, one short line each. '
                   'Plain text, no markdown.',
         'allowed_tools': 'Read'},
    ],
}

# TERM_PROGRAM (passed by the hook) -> app to bring to the front
TERMINAL_APPS = {
    'ghostty': 'Ghostty', 'Apple_Terminal': 'Terminal', 'iTerm.app': 'iTerm',
    'WarpTerminal': 'Warp', 'vscode': 'Visual Studio Code', 'WezTerm': 'WezTerm',
}
WORKING = ('UserPromptSubmit', 'PreToolUse', 'PostToolUse', 'SubagentStop')


def load_config():
    if not CONFIG_PATH.exists():
        cfg = dict(DEFAULT_CONFIG, token=secrets.token_hex(16))
        CONFIG_PATH.write_text(json.dumps(cfg, indent=2) + '\n')
        CONFIG_PATH.chmod(0o600)
        print('created', CONFIG_PATH)
    return json.loads(CONFIG_PATH.read_text())


cfg = load_config()
lock = threading.Lock()
sessions = {}         # session_id -> dict
task_state = {}       # index -> {'state', 'output', 'started', 'finished'}
pico = {'ip': None}   # learned from the Pico's own requests
timer = {'end': None}
CLAUDE = shutil.which('claude') or str(Path.home() / '.local/bin/claude')


# The Pico's font is plain ASCII
_ASCII = {'\u2022': '-', '\u2013': '-', '\u2014': '-', '\u2018': "'", '\u2019': "'",
          '\u201c': '"', '\u201d': '"', '\u2026': '...', '\u00a0': ' ', '\u2192': '->'}


def ascii_text(text):
    for k, v in _ASCII.items():
        text = text.replace(k, v)
    return text.encode('ascii', 'replace').decode()


def osascript(script):
    r = subprocess.run(['osascript', '-e', script], capture_output=True, text=True, timeout=5)
    return r.stdout.strip()


def notify(text):
    osascript('display notification "%s" with title "pico-thing" sound name "Glass"' % text.replace('"', "'"))


def poke_pico():
    """Tell the Pico to poll now instead of waiting for its next 30s check."""
    ip = pico['ip']
    if not ip:
        return

    def send():
        try:
            req = urllib.request.Request('http://%s:%d/poke' % (ip, cfg['pico_port']), b'',
                                         {'X-Token': cfg['token']}, method='POST')
            urllib.request.urlopen(req, timeout=2).close()
        except Exception:
            pass
    threading.Thread(target=send, daemon=True).start()


# --- built-in actions ---

def mic_muted():
    try:
        return osascript('input volume of (get volume settings)') == '0'
    except Exception:
        return False


def toggle_mic():
    if mic_muted():
        osascript('set volume input volume %d' % cfg.get('mic_volume', 75))
    else:
        cfg['mic_volume'] = int(osascript('input volume of (get volume settings)') or 75)
        osascript('set volume input volume 0')


def toggle_focus_timer():
    if timer['end']:
        timer['end'] = None
        return
    end = time.time() + cfg['focus_minutes'] * 60
    timer['end'] = end

    def ring():
        time.sleep(max(0, end - time.time()))
        if timer['end'] == end:
            timer['end'] = None
            notify('Focus session done')
            poke_pico()
    threading.Thread(target=ring, daemon=True).start()


BUILTIN = {
    'mic': ('Mute mic', toggle_mic),
    'focus': ('Focus %dm', toggle_focus_timer),
    'lock': ('Lock screen', lambda: subprocess.run(['pmset', 'displaysleepnow'], timeout=5)),
}


def actions_status():
    out = []
    for aid, (label, _) in BUILTIN.items():
        a = {'id': aid, 'label': label % cfg['focus_minutes'] if '%' in label else label}
        if aid == 'mic':
            a['on'] = mic_muted()
            if a['on']:
                a['label'] = 'Mic muted'
        if aid == 'focus' and timer['end']:
            a['on'] = True
            a['label'] = 'Focus %dm left' % max(1, round((timer['end'] - time.time()) / 60))
        out.append(a)
    for a in cfg.get('actions', []):
        out.append({'id': a['id'], 'label': a['label']})
    return out


def run_action(aid):
    if aid in BUILTIN:
        BUILTIN[aid][1]()
        return True
    for a in cfg.get('actions', []):
        if a['id'] == aid:
            subprocess.Popen(a['cmd'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            return True
    return False


# --- agent tasks ---

def run_task(i):
    t = cfg['tasks'][i]
    with lock:
        if task_state.get(i, {}).get('state') == 'running':
            return
        task_state[i] = {'state': 'running', 'output': '', 'started': time.time()}

    def work():
        try:
            r = subprocess.run([CLAUDE, '-p', t['prompt'], '--allowedTools', t.get('allowed_tools', 'Read')],
                               cwd=os.path.expanduser(t.get('cwd', '~')), capture_output=True,
                               text=True, timeout=t.get('timeout', 600))
            state = 'done' if r.returncode == 0 else 'failed'
            out = (r.stdout or r.stderr).strip()
        except Exception as e:
            state, out = 'failed', str(e)
        with lock:
            task_state[i] = {'state': state, 'output': out, 'finished': time.time()}
        notify('%s %s' % (t['name'], 'finished' if state == 'done' else 'failed'))
        poke_pico()
    threading.Thread(target=work, daemon=True).start()


# --- Claude Code hooks ---

def on_hook(event, term):
    sid = event.get('session_id', '?')
    name = event.get('hook_event_name', '')
    changed = False
    with lock:
        if name == 'SessionEnd':
            sessions.pop(sid, None)
        else:
            s = sessions.setdefault(sid, {'id': sid, 'state': 'idle', 'message': '', 'seen': False})
            s['project'] = os.path.basename(event.get('cwd', '') or '?')
            s['term'] = term or s.get('term', '')
            s['updated'] = time.time()
            before = s['state']
            if name == 'PermissionRequest':
                s['state'], s['message'] = 'waiting', 'Wants to use %s' % event.get('tool_name', 'a tool')
            elif name == 'Notification':
                s['state'], s['message'] = 'waiting', event.get('message', 'Needs your input')
            elif name == 'Stop':
                s['state'], s['message'] = 'done', 'Finished'
            elif name in WORKING:
                s['state'], s['message'] = 'working', ''
            elif name == 'SessionStart':
                s['state'], s['message'] = 'idle', ''
            changed = s['state'] != before
            if changed:
                s['seen'] = False
    if name == 'SessionEnd' or changed:
        poke_pico()


def todo_path():
    return Path(os.path.expanduser(cfg.get('todo_file', str(HERE.parent / 'TODO.md'))))


todos_rev = {'n': 0}


def watch_todos():
    """Poke the Pico whenever the TODO file is saved, so edits show up in seconds."""
    last = None
    while True:
        try:
            mtime = todo_path().stat().st_mtime
        except OSError:
            mtime = None
        if mtime != last:
            if last is not None:
                todos_rev['n'] += 1
                poke_pico()
            last = mtime
        time.sleep(2)


def read_todos():
    """Open items from a markdown checklist ("- [ ] thing"); falls back to ## headings."""
    path = todo_path()
    try:
        text = path.read_text()
    except OSError:
        return {'items': [], 'source': path.name, 'error': 'missing'}
    items = re.findall(r'^\s*[-*] \[ \] (.+)$', text, re.M)
    if not items:
        items = [line[3:] for line in text.splitlines() if line.startswith('## ')]
    clean = [ascii_text(re.sub(r'[*`_]|\[([^]]*)\]\([^)]*\)', lambda m: m.group(1) or '', i).strip())
             for i in items]
    return {'items': [i[:120] for i in clean if i][:12], 'source': path.name}


# --- Mac stats for the Pico's system face ------------------------------------
# Sampled every ~2s, but only while the Pico has asked for them in the last two
# minutes (so `top` isn't running for nothing).
STATS_HIST = 44          # samples kept: one per column of the Pico's sparklines
STATS_IDLE = 120         # stop sampling this long after the last /stats request
stats = {'cpu': [], 'mem': [], 'rx': [], 'tx': [], 'info': {}}
stats_wanted = {'at': 0.0, 'thread': None}


def _out(argv):
    try:
        return subprocess.run(argv, capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.SubprocessError):
        return ''


def _cpu_busy():
    """Whole-machine CPU use (%), from top's second sample (the first is since boot)."""
    lines = [l for l in _out(['top', '-l', '2', '-n', '0', '-s', '1']).splitlines() if 'CPU usage' in l]
    m = re.search(r'([\d.]+)% idle', lines[-1]) if lines else None
    return round(100 - float(m.group(1))) if m else 0


def _memory():
    """(used, total) bytes, used counted like Activity Monitor: active + wired + compressed."""
    out = _out(['vm_stat'])
    page = int((re.search(r'page size of (\d+)', out) or [0, 16384])[1])
    pages = {k.strip(): int(v.strip(' .')) for k, v in re.findall(r'^(Pages [^:]+):\s+(\d+)', out, re.M)}
    used = (pages.get('Pages active', 0) + pages.get('Pages wired down', 0)
            + pages.get('Pages occupied by compressor', 0)) * page
    total = int(_out(['sysctl', '-n', 'hw.memsize']).strip() or 0)
    return used, total


def _net_bytes():
    """(received, sent) bytes so far on the en* interfaces."""
    rx = tx = 0
    for line in _out(['netstat', '-ibn']).splitlines():
        f = line.split()
        if len(f) >= 10 and f[0].startswith('en') and f[2].startswith('<Link'):
            try:
                rx += int(f[-5]); tx += int(f[-2])
            except ValueError:
                pass
    return rx, tx


def _battery():
    m = re.search(r'(\d+)%; ([a-z ]+);', _out(['pmset', '-g', 'batt']))
    return (int(m.group(1)), m.group(2).strip() in ('charging', 'charged', 'finishing charge')) if m else (None, False)


def _uptime():
    m = re.search(r'sec = (\d+)', _out(['sysctl', '-n', 'kern.boottime']))
    return int(time.time() - int(m.group(1))) if m else 0


def _push(key, v):
    stats[key] = (stats[key] + [v])[-STATS_HIST:]


def sample_stats():
    last_net = None
    while True:
        if time.time() - stats_wanted['at'] > STATS_IDLE:
            last_net = None
            time.sleep(2)
            continue
        t0 = time.time()
        cpu = _cpu_busy()  # takes about a second
        used, total = _memory()
        rx, tx = _net_bytes()
        now = time.time()
        rx_k = tx_k = 0
        if last_net:
            dt = max(now - last_net[0], 0.1)
            rx_k = max(0, round((rx - last_net[1]) / dt / 1024))
            tx_k = max(0, round((tx - last_net[2]) / dt / 1024))
        last_net = (now, rx, tx)
        battery, charging = _battery()
        disk = shutil.disk_usage('/System/Volumes/Data' if os.path.exists('/System/Volumes/Data') else '/')
        with lock:
            _push('cpu', cpu)
            _push('mem', round(100 * used / total) if total else 0)
            _push('rx', rx_k)
            _push('tx', tx_k)
            stats['info'] = {
                'mem_used': round(used / 2**30, 1), 'mem_total': round(total / 2**30),
                'load': [round(x, 2) for x in os.getloadavg()],
                'battery': battery, 'charging': charging,
                'disk_used': round(disk.used / 1e9), 'disk_total': round(disk.total / 1e9),
                'uptime': _uptime(), 'cores': os.cpu_count() or 0,
                'host': ascii_text(os.uname().nodename.split('.')[0])[:16],
            }
        time.sleep(max(0.0, 2 - (time.time() - t0)))


def stats_reply():
    stats_wanted['at'] = time.time()
    if stats_wanted['thread'] is None:
        stats_wanted['thread'] = threading.Thread(target=sample_stats, daemon=True)
        stats_wanted['thread'].start()
    with lock:
        return {'cpu': stats['cpu'], 'mem': stats['mem'], 'rx': stats['rx'], 'tx': stats['tx'], **stats['info']}


def status():
    with lock:
        ss = sorted(sessions.values(), key=lambda s: -s['updated'])[:6]
        attention = [s for s in ss if s['state'] in ('waiting', 'done') and not s['seen']]
        return {
            # Kept small: the Pico parses this in ~70KB of RAM
            'sessions': [{'id': s['id'], 'project': ascii_text(s['project'][:20]), 'state': s['state'],
                          'message': ascii_text(s['message'][:90])} for s in ss],
            'attention': attention[0]['id'] if attention else None,
            'todos_rev': todos_rev['n'],
            'tasks': [{'name': t['name'][:24], 'state': task_state.get(i, {}).get('state', 'idle')}
                      for i, t in enumerate(cfg['tasks'])],
            'actions': actions_status(),
        }


class Handler(BaseHTTPRequestHandler):
    server_version = 'pico-thing'

    def log_message(self, fmt, *args):
        pass

    def _reply(self, code, obj=None):
        body = json.dumps(obj).encode() if obj is not None else b''
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _authed(self):
        if secrets.compare_digest(self.headers.get('X-Token', ''), cfg['token']):
            return True
        self._reply(403, {'error': 'bad token'})
        return False

    def do_GET(self):
        if not self._authed():
            return
        url = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(url.query).items()}
        if not self.client_address[0].startswith('127.'):
            pico['ip'] = self.client_address[0]
        if url.path == '/status':
            self._reply(200, status())
        elif url.path == '/stats':
            self._reply(200, stats_reply())
        elif url.path == '/todos':
            self._reply(200, read_todos())
        elif url.path == '/task':
            i = int(q.get('n', -1))
            if not 0 <= i < len(cfg['tasks']):
                return self._reply(404, {'error': 'no such task'})
            st = task_state.get(i, {'state': 'idle', 'output': ''})
            # The Pico pages through this; cap it so the response stays small
            self._reply(200, {'name': cfg['tasks'][i]['name'], 'state': st['state'],
                              'output': ascii_text(st.get('output', '')[:1800])})
        else:
            self._reply(404, {'error': 'not found'})

    def do_POST(self):
        if not self._authed():
            return
        url = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(url.query).items()}
        body = self.rfile.read(int(self.headers.get('Content-Length') or 0))
        if url.path == '/hook':
            try:
                on_hook(json.loads(body or b'{}'), self.headers.get('X-Term', ''))
            except ValueError:
                return self._reply(400, {'error': 'bad json'})
            return self._reply(204)
        if url.path == '/focus':
            s = sessions.get(q.get('id', ''))
            app = TERMINAL_APPS.get((s or {}).get('term', ''), 'Ghostty')
            subprocess.Popen(['open', '-a', app])
            if s:
                s['seen'] = True
            return self._reply(204)
        if url.path == '/dismiss':
            with lock:
                s = sessions.get(q.get('id', ''))
                if s:
                    s['seen'] = True
            return self._reply(204)
        if url.path == '/notify':
            notify(ascii_text(q.get('text', 'pico-thing'))[:80])
            return self._reply(204)
        if url.path == '/action':
            return self._reply(204 if run_action(q.get('id', '')) else 404)
        if url.path == '/run':
            i = int(q.get('n', -1))
            if not 0 <= i < len(cfg['tasks']):
                return self._reply(404, {'error': 'no such task'})
            run_task(i)
            return self._reply(204)
        self._reply(404, {'error': 'not found'})


if __name__ == '__main__':
    threading.Thread(target=watch_todos, daemon=True).start()
    srv = ThreadingHTTPServer(('0.0.0.0', cfg['port']), Handler)
    print('pico-thing companion on port %d' % cfg['port'])
    srv.serve_forever()
