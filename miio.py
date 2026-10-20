# Xiaomi miIO protocol (AES-encrypted UDP 54321) for Mi-ecosystem Yeelights such
# as the bslamp2, which the Yeelight app won't open up for LAN Control. Commands
# are the same JSON methods as Yeelight LAN; only the transport differs.
# Needs the device's 32-hex-char token (see config.MIIO_TOKENS).
import binascii
import cryptolib
import hashlib
import json
import socket
import struct
import time
import net
from yeelight import Light

PORT = 54321
TIMEOUT_MS = 2000
_HELLO = b'\x21\x31\x00\x20' + b'\xff' * 28
_CBC = 2
SUPPORT = 'get_prop set_power toggle set_bright set_ct_abx set_rgb start_cf stop_cf'
_seq = time.time() & 0xFFFF     # devices drop repeated ids, so don't restart at 1 each boot


def _md5(b):
    return hashlib.md5(b).digest()


class MiioLight(Light):
    def __init__(self, ip, token, model='bslamp2', name=''):
        super().__init__(ip, 'miio:' + ip, model, SUPPORT, name)
        self.token = binascii.unhexlify(token)
        self.key = _md5(self.token)
        self.iv = _md5(self.key + self.token)
        self._did = None
        self._stamp = 0
        self._t0 = 0

    async def _handshake(self, s, addr):
        s.sendto(_HELLO, addr)
        r = await net.udp_recv(s, TIMEOUT_MS, 64)
        if not r:
            raise OSError("%s didn't answer" % self.label)
        data = r[0]
        self._did = data[8:12]
        self._stamp = struct.unpack('>I', data[12:16])[0]
        self._t0 = time.time()

    async def call(self, method, *params):
        global _seq
        _seq += 1
        seq = _seq
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.setblocking(False)
        try:
            addr = socket.getaddrinfo(self.ip, PORT)[0][-1]
            if self._did is None or time.time() - self._t0 > 120:
                await self._handshake(s, addr)
            payload = json.dumps({'id': seq, 'method': method, 'params': list(params)}).encode()
            pad = 16 - len(payload) % 16
            enc = cryptolib.aes(self.key, _CBC, self.iv).encrypt(payload + bytes([pad]) * pad)
            stamp = self._stamp + time.time() - self._t0
            header = struct.pack('>HHI', 0x2131, 32 + len(enc), 0) + self._did + struct.pack('>I', stamp)
            s.sendto(header + _md5(header + self.token + enc) + enc, addr)
            deadline = time.ticks_add(time.ticks_ms(), TIMEOUT_MS)
            while True:
                left = time.ticks_diff(deadline, time.ticks_ms())
                r = await net.udp_recv(s, max(left, 0))
                if not r:
                    raise OSError("%s didn't answer" % self.label)
                data = r[0]
                if len(data) <= 32:
                    continue
                dec = cryptolib.aes(self.key, _CBC, self.iv).decrypt(data[32:])
                reply = json.loads(dec[:-dec[-1]].rstrip(b'\x00'))
                if reply.get('id') == seq:
                    if 'error' in reply:
                        raise OSError(reply['error'].get('message', 'error'))
                    self.online = True
                    return reply.get('result')
        except Exception:
            self.online = False
            self._did = None
            raise
        finally:
            s.close()
