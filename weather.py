# Current weather from Open-Meteo (no API key). Location is auto-detected
# from your IP unless set in config.py; the UTC offset comes along for free.
import time
import config
import net


def describe(code, day=True):
    """WMO weather code -> (label, icon name)."""
    if code is None:
        return 'Unknown', 'cloud'
    if code <= 1:
        return ('Clear' if code == 0 else 'Mostly clear'), ('sun' if day else 'moon')
    if code == 2:
        return 'Partly cloudy', ('partly' if day else 'partly_night')
    if code == 3:
        return 'Overcast', 'cloud'
    if code in (45, 48):
        return 'Fog', 'fog'
    if 51 <= code <= 57:
        return 'Drizzle', 'drizzle'
    if 61 <= code <= 67:
        return 'Rain', 'rain'
    if 80 <= code <= 82:
        return 'Showers', 'rain'
    if 71 <= code <= 77 or code in (85, 86):
        return 'Snow', 'snow'
    if code >= 95:
        return 'Thunderstorm', 'storm'
    return 'Cloudy', 'cloud'


class Weather:
    def __init__(self):
        self.lat = getattr(config, 'LATITUDE', None)
        self.lon = getattr(config, 'LONGITUDE', None)
        self.city = getattr(config, 'CITY', None) or ''
        self.fahrenheit = getattr(config, 'UNITS', 'C') == 'F'
        self.offset = 0          # seconds east of UTC
        self.ok = False
        self.updated = None      # local "HH:MM" of last successful fetch
        self.temp = self.feels = self.hi = self.lo = None
        self.humidity = self.wind = self.rain = None
        self.code = None
        self.day = True
        self.sunrise = self.sunset = ''

    @property
    def label(self):
        return describe(self.code, self.day)[0]

    @property
    def icon(self):
        return describe(self.code, self.day)[1]

    async def locate(self):
        j = await net.get_json('http://ip-api.com/json/?fields=status,lat,lon,city,offset')
        if j.get('status') != 'success':
            return False
        self.lat, self.lon = j['lat'], j['lon']
        self.city = self.city or j.get('city', '')
        self.offset = j.get('offset', 0)
        return True

    async def update(self):
        try:
            if self.lat is None and not await self.locate():
                return False
            url = ('http://api.open-meteo.com/v1/forecast?latitude=%.4f&longitude=%.4f'
                   '&current=temperature_2m,apparent_temperature,relative_humidity_2m,'
                   'weather_code,wind_speed_10m,is_day'
                   '&daily=temperature_2m_max,temperature_2m_min,'
                   'precipitation_probability_max,sunrise,sunset'
                   '&timezone=auto&forecast_days=1' % (self.lat, self.lon))
            if self.fahrenheit:
                url += '&temperature_unit=fahrenheit&wind_speed_unit=mph'
            j = await net.get_json(url)
            cur, daily = j['current'], j['daily']
            self.offset = j.get('utc_offset_seconds', self.offset)
            self.temp = cur['temperature_2m']
            self.feels = cur['apparent_temperature']
            self.humidity = cur['relative_humidity_2m']
            self.wind = cur['wind_speed_10m']
            self.code = cur['weather_code']
            self.day = bool(cur['is_day'])
            self.hi = daily['temperature_2m_max'][0]
            self.lo = daily['temperature_2m_min'][0]
            self.rain = daily['precipitation_probability_max'][0]
            self.sunrise = daily['sunrise'][0][11:16]
            self.sunset = daily['sunset'][0][11:16]
            t = time.localtime(time.time() + self.offset)
            self.updated = '%02d:%02d' % (t[3], t[4])
            self.ok = True
            return True
        except Exception as e:
            print('weather failed:', e)
            return False
