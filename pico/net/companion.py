# Client for the Mac companion server, plus the tiny listener it pokes when
# something changes (so attention prompts show up instantly instead of on the
# next 30s poll).
import asyncio
import json
import ahttp
import config

BASE = getattr(config, 'COMPANION', None)
TOKEN = getattr(config, 'COMPANION_TOKEN', '')
LISTEN_PORT = 8080


async def call(method, path, timeout=4):
    if not BASE:
        raise OSError('COMPANION not set in config.py')
    status, body = await ahttp.request(method, BASE + path, b'' if method == 'POST' else None,
                                       {'X-Token': TOKEN}, timeout)
    if status == 403:
        raise OSError('companion rejected the token')
    if status >= 300:
        raise OSError('companion error %d' % status)
    return json.loads(body.decode()) if body else None


async def status():
    return await call('GET', '/status')


async def serve(on_poke):
    """Accept POST /poke from the companion; anything else is refused."""
    async def handle(reader, writer):
        ok = False
        try:
            req = await asyncio.wait_for(reader.readline(), 2)
            while True:
                line = await asyncio.wait_for(reader.readline(), 2)
                if line in (b'\r\n', b''):
                    break
                k, _, v = line.partition(b':')
                if k.strip().lower() == b'x-token' and v.strip().decode() == TOKEN:
                    ok = True
            ok = ok and req.startswith(b'POST /poke')
            writer.write(b'HTTP/1.0 204 No Content\r\n\r\n' if ok else b'HTTP/1.0 403 Forbidden\r\n\r\n')
            await writer.drain()
        except Exception:
            pass
        finally:
            await writer.wait_closed()
        if ok:
            on_poke()
    return await asyncio.start_server(handle, '0.0.0.0', LISTEN_PORT)
