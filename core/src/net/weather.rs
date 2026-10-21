//! Current weather from Open-Meteo (no API key). The location comes from config, or
//! from the IP address via ip-api; the UTC offset comes along for free.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::Deserialize;

use super::{Net, NetError, http};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Sun,
    Moon,
    Partly,
    PartlyNight,
    Cloud,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Storm,
}

#[derive(Clone, Debug, Default)]
pub struct Weather {
    pub lat: f32,
    pub lon: f32,
    pub city: String,
    /// Seconds east of UTC at that location
    pub utc_offset: i32,
    pub temp: f32,
    pub feels: f32,
    pub hi: f32,
    pub lo: f32,
    pub humidity: f32,
    pub wind: f32,
    pub rain: f32,
    pub code: u8,
    pub day: bool,
    pub sunrise: String,
    pub sunset: String,
    pub fahrenheit: bool,
}

impl Weather {
    pub fn label(&self) -> &'static str {
        describe(self.code, self.day).0
    }

    pub fn icon(&self) -> Icon {
        describe(self.code, self.day).1
    }
}

/// WMO weather code -> (label, icon).
pub fn describe(code: u8, day: bool) -> (&'static str, Icon) {
    match code {
        0 => ("Clear", if day { Icon::Sun } else { Icon::Moon }),
        1 => ("Mostly clear", if day { Icon::Sun } else { Icon::Moon }),
        2 => ("Partly cloudy", if day { Icon::Partly } else { Icon::PartlyNight }),
        3 => ("Overcast", Icon::Cloud),
        45 | 48 => ("Fog", Icon::Fog),
        51..=57 => ("Drizzle", Icon::Drizzle),
        61..=67 => ("Rain", Icon::Rain),
        80..=82 => ("Showers", Icon::Rain),
        71..=77 | 85 | 86 => ("Snow", Icon::Snow),
        95.. => ("Thunderstorm", Icon::Storm),
        _ => ("Cloudy", Icon::Cloud),
    }
}

#[derive(Deserialize)]
struct IpApi {
    status: String,
    #[serde(default)]
    lat: f32,
    #[serde(default)]
    lon: f32,
    #[serde(default)]
    city: String,
    #[serde(default)]
    offset: i32,
}

#[derive(Deserialize)]
struct Forecast {
    #[serde(default)]
    utc_offset_seconds: i32,
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: f32,
    apparent_temperature: f32,
    relative_humidity_2m: f32,
    weather_code: u8,
    wind_speed_10m: f32,
    is_day: u8,
}

#[derive(Deserialize)]
struct Daily {
    temperature_2m_max: Vec<f32>,
    temperature_2m_min: Vec<f32>,
    precipitation_probability_max: Vec<Option<f32>>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
}

/// Where we are: configured coordinates, or looked up from our public IP.
pub async fn locate<N: Net>(net: &mut N) -> Result<(f32, f32, String, i32), NetError> {
    let r = http::get(net, "http://ip-api.com/json/?fields=status,lat,lon,city,offset", &[]).await?.ok()?;
    let j: IpApi = serde_json::from_slice(&r.body).map_err(|_| NetError::Protocol)?;
    if j.status != "success" {
        return Err(NetError::Protocol);
    }
    Ok((j.lat, j.lon, j.city, j.offset))
}

/// Fetch the current weather. `place` is (lat, lon, city) when already known.
pub async fn fetch<N: Net>(net: &mut N, place: Option<(f32, f32, String)>, fahrenheit: bool) -> Result<Weather, NetError> {
    let (lat, lon, city) = match place {
        Some(p) => p,
        None => {
            let (lat, lon, city, _) = locate(net).await?;
            (lat, lon, city)
        }
    };
    let mut url = format!(
        "http://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m,is_day\
         &daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset\
         &timezone=auto&forecast_days=1"
    );
    if fahrenheit {
        url.push_str("&temperature_unit=fahrenheit&wind_speed_unit=mph");
    }
    let r = http::get(net, &url, &[]).await?.ok()?;
    let j: Forecast = serde_json::from_slice(&r.body).map_err(|_| NetError::Protocol)?;
    let first = |v: &Vec<f32>| v.first().copied().unwrap_or(0.0);
    let hhmm = |v: &Vec<String>| v.first().and_then(|s| s.get(11..16)).unwrap_or("").into();
    let c = &j.current;
    Ok(Weather {
        lat,
        lon,
        city,
        utc_offset: j.utc_offset_seconds,
        temp: c.temperature_2m,
        feels: c.apparent_temperature,
        hi: first(&j.daily.temperature_2m_max),
        lo: first(&j.daily.temperature_2m_min),
        humidity: c.relative_humidity_2m,
        wind: c.wind_speed_10m,
        rain: j.daily.precipitation_probability_max.first().copied().flatten().unwrap_or(0.0),
        code: c.weather_code,
        day: c.is_day != 0,
        sunrise: hhmm(&j.daily.sunrise),
        sunset: hhmm(&j.daily.sunset),
        fahrenheit,
    })
}
