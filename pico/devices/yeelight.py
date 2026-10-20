# Yeelight LAN control. Each light needs "LAN Control" enabled in the Yeelight app.
# Protocol: SSDP-style discovery on UDP 1982, then JSON lines over TCP 55443.
import asyncio
import json
import socket
import time
import net

PORT = 55443
TIMEOUT = 3
_SEARCH = (b'M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1982\r\n'
           b'MAN: "ssdp:discover"\r\nST: wifi_bulb\r\n')
MODEL_NAMES = {
    'mono': 'Bulb', 'mono1': 'Bulb', 'mono4': 'Bulb', 'mono5': 'Bulb', 'mono6': 'Bulb',
    'color': 'Colour bulb', 'color1': 'Colour bulb', 'color2': 'Colour bulb',
    'color4': 'Colour bulb', 'ct_bulb': 'Bulb', 'stripe': 'Light strip', 'strip6': 'Light strip',
    'bslamp1': 'Bedside lamp', 'bslamp2': 'Bedside lamp', 'bslamp3': 'Bedside lamp',
    'ceiling': 'Ceiling light', 'lamp': 'Desk lamp',
}
_seq = 0


class Light:
    def __init__(self, ip, lid='', model='', support='', name=''):
        self.ip = ip
        self.id = lid
        self.model = model
        self.support = support.split() if isinstance(support, str) else list(support)
        self.name = name
        self.power = False
        self.bright = 0
        self.ct = 0
        self.online = None      # None = not checked yet

    @classmethod
    def from_dict(cls, d):
        return cls(d['ip'], d.get('id', ''), d.get('model', ''), d.get('support', ()), d.get('name', ''))

    def to_dict(self):
        return {'ip': self.ip, 'id': self.id, 'model': self.model,
                'support': self.support, 'name': self.name}

    @property
    def label(self):
        return self.name or MODEL_NAMES.get(self.model, self.model or self.ip)

    def can(self, method):
        return method in self.support

    async def call(self, method, *params):
        global _seq
        _seq += 1
        try:
            return await asyncio.wait_for(self._call(_seq, method, params), TIMEOUT)
        except asyncio.TimeoutError:
            self.online = False
            raise OSError("%s didn't answer" % self.label)
        except Exception:
            self.online = False
            raise

    async def _call(self, seq, method, params):
        reader, writer = await asyncio.open_connection(self.ip, PORT)
        try:
            msg = json.dumps({'id': seq, 'method': method, 'params': list(params)}) + '\r\n'
            writer.write(msg.encode())
            await writer.drain()
            while True:
                line = await reader.readline()
                if not line:
                    raise OSError('no reply')
                r = json.loads(line)
                # Replies can be interleaved with unsolicited "props" notifications
                if r.get('id') == seq:
                    if 'error' in r:
                        raise OSError(r['error'].get('message', 'error'))
                    self.online = True
                    return r.get('result')
        finally:
            # MicroPython's StreamWriter.close() is a no-op; wait_closed() is what
            # actually closes the socket (and it doesn't yield, so it's safe on cancel)
            await writer.wait_closed()

    async def refresh(self):
        power, bright, ct, name = await self.call('get_prop', 'power', 'bright', 'ct', 'name')
        self.power = power == 'on'
        self.bright = int(bright or 0)
        self.ct = int(ct or 0)
        self.name = name or self.name

    async def set_power(self, on):
        await self.call('set_power', 'on' if on else 'off', 'smooth', 300)
        self.power = on

    async def set_bright(self, pct):
        pct = min(max(int(pct), 1), 100)
        if not self.power:
            await self.set_power(True)
        await self.call('set_bright', pct, 'smooth', 300)
        self.bright = pct

    async def set_ct(self, kelvin):
        if not self.power:
            await self.set_power(True)
        await self.call('set_ct_abx', kelvin, 'smooth', 300)
        self.ct = kelvin

    async def identify(self):
        """Blink a few times so you can see which light this is."""
        was_on = self.power
        if not was_on:
            await self.set_power(True)
        # Flow of (duration ms, mode 2 = colour temp, kelvin, brightness); action 0 = restore after
        await self.call('start_cf', 6, 0, '350,2,4000,100,350,2,4000,1')
        if not was_on:
            await asyncio.sleep_ms(2300)
            await self.set_power(False)

    async def set_rgb(self, rgb):
        if not self.power:
            await self.set_power(True)
        await self.call('set_rgb', rgb, 'smooth', 300)


async def discover(timeout=3):
    """Broadcast a search and collect replies -> list of Light."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.setblocking(False)
    found = {}
    try:
        for _ in range(2):
            s.sendto(_SEARCH, ('239.255.255.250', 1982))
        end = time.ticks_add(time.ticks_ms(), timeout * 1000)
        while True:
            left = time.ticks_diff(end, time.ticks_ms())
            if left <= 0:
                break
            r = await net.udp_recv(s, left)
            if not r:
                break
            h = {}
            for line in r[0].decode().split('\r\n')[1:]:
                k, sep, v = line.partition(':')
                if sep:
                    h[k.strip().lower()] = v.strip()
            loc = h.get('location', '')
            if not loc.startswith('yeelight://'):
                continue
            ip = loc[11:].split(':')[0]
            light = Light(ip, h.get('id', ''), h.get('model', ''), h.get('support', ''), h.get('name', ''))
            light.power = h.get('power') == 'on'
            light.bright = int(h.get('bright') or 0)
            light.online = True
            found[light.id or ip] = light
    finally:
        s.close()
    return list(found.values())
