# Minimal async HTTP/1.0 client on asyncio streams (MicroPython's `requests` blocks
# the whole UI). HTTP/1.0 keeps servers from using chunked encoding; the whole
# response is read into memory, so keep responses small.
import asyncio
import json


async def request(method, url, body=None, headers=None, timeout=4):
    """-> (status, body bytes). Raises OSError on network failure or timeout."""
    try:
        return await asyncio.wait_for(_request(method, url, body, headers), timeout)
    except asyncio.TimeoutError:
        raise OSError(110, 'timed out')


async def _request(method, url, body, headers):
    if not url.startswith('http://'):
        raise ValueError('only http:// is supported')
    hostport, _, path = url[7:].partition('/')
    host, _, port = hostport.partition(':')
    reader, writer = await asyncio.open_connection(host, int(port) if port else 80)
    try:
        if isinstance(body, str):
            body = body.encode()
        lines = ['%s /%s HTTP/1.0' % (method, path), 'Host: ' + hostport, 'Connection: close']
        if headers:
            lines += ['%s: %s' % kv for kv in headers.items()]
        if body is not None:
            lines.append('Content-Length: %d' % len(body))
        writer.write(('\r\n'.join(lines) + '\r\n\r\n').encode())
        if body:
            writer.write(body)
        await writer.drain()
        chunks = []
        while True:
            c = await reader.read(1024)
            if not c:
                break
            chunks.append(c)
        data = b''.join(chunks)
        del chunks
    finally:
        # MicroPython's StreamWriter.close() is a no-op; wait_closed() is what
        # actually closes the socket (and it doesn't yield, so it's safe on cancel)
        await writer.wait_closed()
    end = data.find(b'\r\n\r\n')
    if end < 0:
        raise OSError('bad HTTP response')
    head = data[:end].lower()
    status = int(head.split(b' ', 2)[1])
    body = data[end + 4:]
    # A dropped connection looks like a normal EOF, so check we got everything
    i = head.find(b'content-length:')
    if i >= 0:
        want = int(head[i + 15:].split(b'\r\n', 1)[0])
        if len(body) < want:
            raise OSError('truncated response (%d of %d bytes)' % (len(body), want))
    return status, body


async def get_json(url, timeout=6):
    status, body = await request('GET', url, timeout=timeout)
    if status != 200:
        raise OSError('HTTP %d' % status)
    return json.loads(body.decode())
