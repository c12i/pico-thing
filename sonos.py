# Sonos control over UPnP/SOAP on port 1400.
import gc
import ahttp

AVT = ('/MediaRenderer/AVTransport/Control', 'urn:schemas-upnp-org:service:AVTransport:1')
RC = ('/MediaRenderer/RenderingControl/Control', 'urn:schemas-upnp-org:service:RenderingControl:1')
CD = ('/MediaServer/ContentDirectory/Control', 'urn:schemas-upnp-org:service:ContentDirectory:1')
_MASTER = '<Channel>Master</Channel>'
_INSTANCE = '<InstanceID>0</InstanceID>'
_ENVELOPE = ('<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" '
             's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body>'
             '<u:%s xmlns:u="%s">%s</u:%s></s:Body></s:Envelope>')
# Favorites with these URIs are streams: set them as the source directly.
# Everything else (playlists, albums, tracks) goes through the queue.
STREAM_PREFIXES = ('x-sonosapi-radio:', 'x-sonosapi-stream:', 'x-sonosapi-hls:',
                   'x-rincon-mp3radio:', 'aac:', 'hls-radio:')
# Starting a stream makes the speaker call out to the music service first
SLOW = 12
FAV_PAGE = 2     # ~1.3KB of XML per item; bigger pages fail to find contiguous RAM
# TrackURI prefix -> what to call the source when there's no track title
SOURCES = (('x-sonos-htastream', 'TV'), ('x-rincon-stream', 'Line-in'),
           ('x-sonos-vli', 'AirPlay / Connect'), ('x-sonos-spotify', 'Spotify'),
           ('x-rincon-mp3radio', 'Radio'), ('x-sonosapi-stream', 'Radio'),
           ('x-sonosapi-radio', 'Radio'), ('aac:', 'Radio'), ('hls-radio:', 'Radio'))


def _tag(xml, name):
    i = xml.find('<' + name)
    if i < 0:
        return ''
    i = xml.find('>', i) + 1
    j = xml.find('</' + name, i)
    return xml[i:j] if j >= 0 else ''


def _unescape(s):
    return (s.replace('&lt;', '<').replace('&gt;', '>').replace('&quot;', '"')
             .replace('&apos;', "'").replace('&amp;', '&'))


class Speaker:
    def __init__(self, name, ip):
        self.name = name
        self.ip = ip
        self.uuid = ''
        self.online = None
        self.leader = None       # Speaker this one is grouped under, if any
        self.state = ''          # PLAYING, PAUSED_PLAYBACK, STOPPED, TRANSITIONING
        self.title = self.artist = self.source = ''
        self.volume = 0
        self.muted = False

    async def soap(self, service, action, args='', instance=True, timeout=4):
        path, urn = service
        if instance:
            args = _INSTANCE + args
        body = _ENVELOPE % (action, urn, args, action)
        headers = {'Content-Type': 'text/xml; charset="utf-8"', 'SOAPACTION': '"%s#%s"' % (urn, action)}
        url = 'http://%s:1400%s' % (self.ip, path)
        for attempt in (0, 1):
            try:
                status, data = await ahttp.request('POST', url, body, headers, timeout)
                break
            except OSError as e:
                # EHOSTUNREACH happens before anything is sent (e.g. ARP not resolved
                # yet just after boot), so retrying can't repeat an action
                if attempt == 0 and e.errno == 113:
                    continue
                self.online = False
                if e.errno == 110:
                    raise OSError("%s didn't answer (%s)" % (self.name, action))
                raise
        text = data.decode()
        del data
        self.online = True
        if status != 200:
            raise OSError('%s failed (UPnP error %s)' % (action, _tag(text, 'errorCode') or status))
        return text

    async def load_uuid(self):
        _, data = await ahttp.request('GET', 'http://%s:1400/status/zp' % self.ip)
        self.uuid = _tag(data.decode(), 'LocalUID')

    @property
    def target(self):
        """Transport commands must go to the group coordinator."""
        return self.leader or self

    @property
    def can_skip(self):
        """TV and line-in can't be paused or skipped (UPnP error 701)."""
        return self.source not in ('TV', 'Line-in')

    @property
    def playing(self):
        return self.state in ('PLAYING', 'TRANSITIONING')

    async def refresh(self, speakers):
        if not self.uuid:
            await self.load_uuid()
        x = await self.soap(AVT, 'GetPositionInfo')
        uri = _unescape(_tag(x, 'TrackURI'))
        self.leader = None
        if uri.startswith('x-rincon:'):
            for s in speakers:
                if s is not self and s.uuid == uri[9:]:
                    self.leader = s
                    x = await s.soap(AVT, 'GetPositionInfo')
                    uri = _unescape(_tag(x, 'TrackURI'))
        meta = _unescape(_tag(x, 'TrackMetaData'))
        self.title = _unescape(_tag(meta, 'dc:title'))
        self.artist = _unescape(_tag(meta, 'dc:creator'))
        stream = _unescape(_tag(meta, 'r:streamContent'))
        if stream:
            self.artist = self.artist or stream
        self.source = ''
        for prefix, label in SOURCES:
            if uri.startswith(prefix):
                self.source = label
                break
        self.state = _tag(await self.target.soap(AVT, 'GetTransportInfo'), 'CurrentTransportState')
        self.volume = int(_tag(await self.soap(RC, 'GetVolume', _MASTER), 'CurrentVolume') or 0)
        self.muted = _tag(await self.soap(RC, 'GetMute', _MASTER), 'CurrentMute') == '1'

    async def play(self):
        await self.target.soap(AVT, 'Play', '<Speed>1</Speed>', timeout=SLOW)
        self.state = 'PLAYING'

    async def pause(self):
        await self.target.soap(AVT, 'Pause')
        self.state = 'PAUSED_PLAYBACK'

    async def next(self):
        await self.target.soap(AVT, 'Next', timeout=SLOW)

    async def previous(self):
        await self.target.soap(AVT, 'Previous', timeout=SLOW)

    async def set_volume(self, v):
        v = min(max(int(v), 0), 100)
        await self.soap(RC, 'SetVolume', _MASTER + '<DesiredVolume>%d</DesiredVolume>' % v)
        self.volume = v

    async def set_mute(self, on):
        await self.soap(RC, 'SetMute', _MASTER + '<DesiredMute>%d</DesiredMute>' % (1 if on else 0))
        self.muted = on

    async def join(self, other):
        """Follow whatever `other` is playing."""
        await self.soap(AVT, 'SetAVTransportURI',
                  '<CurrentURI>x-rincon:%s</CurrentURI><CurrentURIMetaData></CurrentURIMetaData>' % other.uuid)
        self.leader = other

    async def leave(self):
        await self.soap(AVT, 'BecomeCoordinatorOfStandaloneGroup')
        self.leader = None

    # --- Sonos Favorites ---

    async def browse(self, oid, start, count):
        """-> (total, [(title, uri, metadata)]). uri/metadata stay XML-escaped,
        ready to embed straight back into a SOAP request."""
        x = await self.soap(CD, 'Browse',
                      '<ObjectID>%s</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag>'
                      '<Filter>*</Filter><StartingIndex>%d</StartingIndex>'
                      '<RequestedCount>%d</RequestedCount><SortCriteria></SortCriteria>'
                      % (oid, start, count), instance=False)
        total = int(_tag(x, 'TotalMatches') or 0)
        didl = _unescape(_tag(x, 'Result'))
        del x
        items = []
        pos = 0
        while True:
            i = didl.find('<item', pos)
            if i < 0:
                break
            j = didl.find('</item>', i)
            chunk = didl[i:j]
            items.append((_unescape(_tag(chunk, 'dc:title')), _tag(chunk, 'res'), _tag(chunk, 'r:resMD')))
            pos = j
        return total, items

    async def _browse_retry(self, oid, start, count):
        gc.collect()        # each page needs a few contiguous KB
        try:
            return await self.browse(oid, start, count)
        except OSError:
            gc.collect()
            return await self.browse(oid, start, count)

    async def favorites(self, progress=None):
        """All playable favorites as [(index, title, is_mix)], fetched a page at a time
        (the full list can be tens of KB). is_mix = station or playlist, vs a single track."""
        favs = []
        start, total = 0, 1
        while start < total:
            if progress:
                progress(start, total)
            page_total, items = await self._browse_retry('FV:2', start, FAV_PAGE)
            # Trust the first real total; a glitchy page mustn't end the list early
            if page_total and (total == 1 or page_total > total):
                total = page_total
            for k, (title, uri, _) in enumerate(items):
                if uri:      # shortcuts without a URI can't be played this way
                    mix = uri.startswith(STREAM_PREFIXES) or uri.startswith('x-rincon-cpcontainer:')
                    favs.append((start + k, title, mix))
            if not items:
                break
            start += len(items)
        return favs

    async def play_favorite(self, index, expect_title=None):
        """Returns False (and plays nothing) if the favorite at `index` is no longer
        `expect_title`, i.e. favorites changed since the list was cached."""
        _, items = await self._browse_retry('FV:2', index, 1)
        if not items or (expect_title and items[0][0] != expect_title):
            return False
        title, uri, meta = items[0]
        t = self.target
        if uri.startswith(STREAM_PREFIXES):
            await t.soap(AVT, 'SetAVTransportURI',
                   '<CurrentURI>%s</CurrentURI><CurrentURIMetaData>%s</CurrentURIMetaData>' % (uri, meta),
                   timeout=SLOW)
        else:
            await t.soap(AVT, 'RemoveAllTracksFromQueue')
            await t.soap(AVT, 'AddURIToQueue',
                   '<EnqueuedURI>%s</EnqueuedURI><EnqueuedURIMetaData>%s</EnqueuedURIMetaData>'
                   '<DesiredFirstTrackNumberEnqueued>0</DesiredFirstTrackNumberEnqueued>'
                   '<EnqueueAsNext>0</EnqueueAsNext>' % (uri, meta), timeout=SLOW)
            await t.soap(AVT, 'SetAVTransportURI',
                   '<CurrentURI>x-rincon-queue:%s#0</CurrentURI><CurrentURIMetaData></CurrentURIMetaData>' % t.uuid)
        await t.play()
        self.title = title
        return True
