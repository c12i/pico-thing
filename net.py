# Wi-Fi, clock sync and async UDP polling for the network modules.
import asyncio
import machine
import network
import socket
import struct
import time
import config
from ahttp import get_json   # re-exported for weather.py

wlan = network.WLAN(network.STA_IF)
NTP_HOST = 'pool.ntp.org'


async def connect(timeout=15):
    if not config.WIFI_SSID:
        return False
    wlan.active(True)
    try:
        wlan.config(pm=0xA11140)   # disable power saving: snappier LAN replies
    except Exception:
        pass
    if wlan.isconnected():
        return True
    wlan.connect(config.WIFI_SSID, config.WIFI_PASSWORD)
    start = time.ticks_ms()
    while not wlan.isconnected():
        if time.ticks_diff(time.ticks_ms(), start) > timeout * 1000:
            return False
        await asyncio.sleep_ms(200)
    return True


def connected():
    return wlan.isconnected()


def ip():
    return wlan.ifconfig()[0] if wlan.isconnected() else None


async def udp_recv(sock, timeout_ms, size=1024):
    """Poll a non-blocking UDP socket -> (data, addr), or None on timeout."""
    deadline = time.ticks_add(time.ticks_ms(), timeout_ms)
    while True:
        try:
            return sock.recvfrom(size)
        except OSError:
            pass
        if time.ticks_diff(deadline, time.ticks_ms()) <= 0:
            return None
        await asyncio.sleep_ms(20)


async def sync_time():
    """Set the RTC to UTC via SNTP (ntptime does the same, but blocks)."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.setblocking(False)
        q = bytearray(48)
        q[0] = 0x1B                      # LI 0, version 3, client mode
        s.sendto(q, socket.getaddrinfo(NTP_HOST, 123)[0][-1])
        r = await udp_recv(s, 2000, 48)
        if not r:
            return False
        t = struct.unpack('!I', r[0][40:44])[0]
        # NTP counts from 1900; the board's epoch is 1970 (or 2000 on older ports)
        t -= 3155673600 if time.gmtime(0)[0] == 2000 else 2208988800
        tm = time.gmtime(t)
        machine.RTC().datetime((tm[0], tm[1], tm[2], tm[6] + 1, tm[3], tm[4], tm[5], 0))
        return True
    except Exception as e:
        print('ntp failed:', e)
        return False
    finally:
        s.close()
