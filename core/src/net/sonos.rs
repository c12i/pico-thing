//! Sonos control over UPnP/SOAP on port 1400. Transport commands go to the group
//! coordinator, so a speaker that's grouped under another one is followed to it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::{Net, NetError, http};

const PORT: u16 = 1400;
const AVT: (&str, &str) = ("/MediaRenderer/AVTransport/Control", "urn:schemas-upnp-org:service:AVTransport:1");
const RC: (&str, &str) = ("/MediaRenderer/RenderingControl/Control", "urn:schemas-upnp-org:service:RenderingControl:1");
const CD: (&str, &str) = ("/MediaServer/ContentDirectory/Control", "urn:schemas-upnp-org:service:ContentDirectory:1");
const MASTER: &str = "<Channel>Master</Channel>";
const INSTANCE: &str = "<InstanceID>0</InstanceID>";
/// Favorites with these URIs are streams: set them as the source directly. Everything
/// else (playlists, albums, tracks) goes through the queue.
const STREAMS: [&str; 6] =
    ["x-sonosapi-radio:", "x-sonosapi-stream:", "x-sonosapi-hls:", "x-rincon-mp3radio:", "aac:", "hls-radio:"];
/// TrackURI prefix -> what to call the source when there's no track title
const SOURCES: [(&str, &str); 9] = [
    ("x-sonos-htastream", "TV"),
    ("x-rincon-stream", "Line-in"),
    ("x-sonos-vli", "AirPlay / Connect"),
    ("x-sonos-spotify", "Spotify"),
    ("x-rincon-mp3radio", "Radio"),
    ("x-sonosapi-stream", "Radio"),
    ("x-sonosapi-radio", "Radio"),
    ("aac:", "Radio"),
    ("hls-radio:", "Radio"),
];
/// Favorites fetched per Browse call (each is ~1.3KB of XML, which is copied while
/// it's unescaped, so small pages keep the Pico's heap comfortable)
const FAV_PAGE: usize = 4;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpeakerState {
    /// PLAYING, PAUSED_PLAYBACK, STOPPED, TRANSITIONING
    pub transport: String,
    pub title: String,
    pub artist: String,
    pub source: String,
    pub album: String,
    /// What it's playing from (a station, playlist or mix), when the speaker says
    pub from: String,
    pub volume: u8,
    pub muted: bool,
    /// The coordinator's IP when this speaker is grouped under another
    pub leader: Option<String>,
}

impl SpeakerState {
    pub fn playing(&self) -> bool {
        self.transport == "PLAYING" || self.transport == "TRANSITIONING"
    }

    /// TV and line-in can't be paused or skipped (UPnP error 701).
    pub fn can_skip(&self) -> bool {
        self.source != "TV" && self.source != "Line-in"
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SonosCmd {
    Play,
    Pause,
    Next,
    Previous,
    Volume(u8),
    Mute(bool),
    /// Play favorite number n (checked against its title first)
    Favorite(usize),
    /// Join the group of the speaker at this IP
    Join(&'static str),
    /// Leave its group, to play on its own
    Leave,
}

/// (index in Sonos Favorites, title, is a station/playlist rather than one track)
pub type Favorite = (usize, String, bool);

fn tag<'a>(xml: &'a str, name: &str) -> &'a str {
    let open = format!("<{name}");
    // The whole name only: `<upnp:album` must not match `<upnp:albumArtURI`
    let mut from = 0;
    let i = loop {
        let Some(i) = xml[from..].find(&open).map(|i| from + i) else { return "" };
        match xml.as_bytes().get(i + open.len()) {
            Some(b'>' | b' ' | b'/' | b'\t' | b'\r' | b'\n') => break i,
            _ => from = i + 1,
        }
    };
    let Some(gt) = xml[i..].find('>') else { return "" };
    let start = i + gt + 1;
    let close = format!("</{name}>");
    match xml[start..].find(&close) {
        Some(j) => &xml[start..start + j],
        None => "",
    }
}

pub fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

async fn soap<N: Net>(net: &mut N, ip: &str, service: (&str, &str), action: &str, args: &str, instance: bool) -> Result<String, NetError> {
    let (path, urn) = service;
    let args = if instance { format!("{INSTANCE}{args}") } else { args.into() };
    let body = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body>\
         <u:{action} xmlns:u=\"{urn}\">{args}</u:{action}></s:Body></s:Envelope>"
    );
    let soap_action = format!("\"{urn}#{action}\"");
    let url = format!("http://{ip}:{PORT}{path}");
    let headers = [("Content-Type", "text/xml; charset=\"utf-8\""), ("SOAPACTION", soap_action.as_str())];
    let r = http::request(net, "POST", &url, &headers, body.as_bytes()).await?;
    drop(body);
    let status = r.status;
    let text = r.into_text();
    if status != 200 {
        return Err(NetError::Http(tag(&text, "errorCode").parse().unwrap_or(status)));
    }
    Ok(text)
}

async fn uuid<N: Net>(net: &mut N, ip: &str) -> Result<String, NetError> {
    let r = http::get(net, &format!("http://{ip}:{PORT}/status/zp"), &[]).await?.ok()?;
    Ok(tag(&r.into_text(), "LocalUID").into())
}

/// Whoever coordinates this speaker's group: (its IP, its GetPositionInfo reply).
async fn coordinator<N: Net>(net: &mut N, ip: &str, others: &[&str]) -> Result<(String, String), NetError> {
    let x = soap(net, ip, AVT, "GetPositionInfo", "", true).await?;
    let uri = unescape(tag(&x, "TrackURI"));
    if let Some(leader) = uri.strip_prefix("x-rincon:") {
        for other in others.iter().filter(|o| **o != ip) {
            if uuid(net, other).await.is_ok_and(|u| u == leader) {
                let x = soap(net, other, AVT, "GetPositionInfo", "", true).await?;
                return Ok(((*other).into(), x));
            }
        }
    }
    Ok((ip.into(), x))
}

pub async fn state<N: Net>(net: &mut N, ip: &str, others: &[&str]) -> Result<SpeakerState, NetError> {
    let (leader, x) = coordinator(net, ip, others).await?;
    let uri = unescape(tag(&x, "TrackURI"));
    let meta = unescape(tag(&x, "TrackMetaData"));
    let mut artist = unescape(tag(&meta, "dc:creator"));
    let stream = unescape(tag(&meta, "r:streamContent"));
    if artist.is_empty() {
        artist = stream;
    }
    let source = SOURCES.iter().find(|(p, _)| uri.starts_with(p)).map(|(_, l)| *l).unwrap_or("");
    let title = unescape(tag(&meta, "dc:title"));
    let album = unescape(tag(&meta, "upnp:album"));
    drop((meta, x)); // (keep the heap free for the replies below)
    let media = soap(net, &leader, AVT, "GetMediaInfo", "", true).await?;
    let mut from = unescape(tag(&unescape(tag(&media, "CurrentURIMetaData")), "dc:title"));
    drop(media);
    if from == title {
        from.clear();
    }
    let transport = tag(&soap(net, &leader, AVT, "GetTransportInfo", "", true).await?, "CurrentTransportState").into();
    let volume = tag(&soap(net, ip, RC, "GetVolume", MASTER, true).await?, "CurrentVolume").parse().unwrap_or(0);
    let muted = tag(&soap(net, ip, RC, "GetMute", MASTER, true).await?, "CurrentMute") == "1";
    Ok(SpeakerState {
        transport,
        title,
        artist,
        source: source.into(),
        album,
        from,
        volume,
        muted,
        leader: (leader != ip).then_some(leader),
    })
}

/// One page of favorites: (total, [(title, uri, metadata)]); uri and metadata stay
/// XML-escaped, ready to go straight back into a SOAP request.
async fn browse<N: Net>(net: &mut N, ip: &str, start: usize, count: usize) -> Result<(usize, Vec<(String, String, String)>), NetError> {
    let args = format!(
        "<ObjectID>FV:2</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag><Filter>*</Filter>\
         <StartingIndex>{start}</StartingIndex><RequestedCount>{count}</RequestedCount><SortCriteria></SortCriteria>"
    );
    let (total, didl) = {
        let x = soap(net, ip, CD, "Browse", &args, false).await?;
        (tag(&x, "TotalMatches").parse().unwrap_or(0), unescape(tag(&x, "Result")))
    }; // the raw reply is freed before parsing
    let mut items = Vec::new();
    let mut rest = didl.as_str();
    while let Some(i) = rest.find("<item") {
        let end = rest[i..].find("</item>").map(|j| i + j).unwrap_or(rest.len());
        let chunk = &rest[i..end];
        items.push((unescape(tag(chunk, "dc:title")), tag(chunk, "res").into(), tag(chunk, "r:resMD").into()));
        rest = &rest[end..];
    }
    Ok((total, items))
}

pub async fn favorites<N: Net>(net: &mut N, ip: &str) -> Result<Vec<Favorite>, NetError> {
    let mut favs = Vec::new();
    let (mut start, mut total) = (0, 1);
    while start < total {
        let (t, items) = browse(net, ip, start, FAV_PAGE).await?;
        total = t;
        if items.is_empty() {
            break;
        }
        for (k, (title, uri, _)) in items.iter().enumerate() {
            if !uri.is_empty() {
                let mix = STREAMS.iter().any(|p| uri.starts_with(p)) || uri.starts_with("x-rincon-cpcontainer:");
                favs.push((start + k, title.clone(), mix));
            }
        }
        start += items.len();
    }
    Ok(favs)
}

pub async fn command<N: Net>(net: &mut N, ip: &str, others: &[&str], cmd: SonosCmd, expect: &str) -> Result<(), NetError> {
    match cmd {
        SonosCmd::Volume(v) => {
            soap(net, ip, RC, "SetVolume", &format!("{MASTER}<DesiredVolume>{}</DesiredVolume>", v.min(100)), true).await?;
        }
        SonosCmd::Mute(on) => {
            soap(net, ip, RC, "SetMute", &format!("{MASTER}<DesiredMute>{}</DesiredMute>", on as u8), true).await?;
        }
        SonosCmd::Join(leader) => {
            // Following another speaker = playing its stream, `x-rincon:<its id>`
            let id = uuid(net, leader).await?;
            let args = format!("<CurrentURI>x-rincon:{id}</CurrentURI><CurrentURIMetaData></CurrentURIMetaData>");
            soap(net, ip, AVT, "SetAVTransportURI", &args, true).await?;
        }
        SonosCmd::Leave => drop(soap(net, ip, AVT, "BecomeCoordinatorOfStandaloneGroup", "", true).await?),
        _ => {
            let (leader, _) = coordinator(net, ip, others).await?;
            let t = leader.as_str();
            match cmd {
                SonosCmd::Play => drop(soap(net, t, AVT, "Play", "<Speed>1</Speed>", true).await?),
                SonosCmd::Pause => drop(soap(net, t, AVT, "Pause", "", true).await?),
                SonosCmd::Next => drop(soap(net, t, AVT, "Next", "", true).await?),
                SonosCmd::Previous => drop(soap(net, t, AVT, "Previous", "", true).await?),
                SonosCmd::Favorite(n) => play_favorite(net, t, n, expect).await?,
                _ => {}
            }
        }
    }
    Ok(())
}

async fn play_favorite<N: Net>(net: &mut N, ip: &str, index: usize, expect: &str) -> Result<(), NetError> {
    let (_, items) = browse(net, ip, index, 1).await?;
    // Favorites may have changed since the list was fetched
    let Some((title, uri, meta)) = items.into_iter().next() else { return Err(NetError::Protocol) };
    if !expect.is_empty() && title != expect {
        return Err(NetError::Protocol);
    }
    if STREAMS.iter().any(|p| uri.starts_with(p)) {
        let args = format!("<CurrentURI>{uri}</CurrentURI><CurrentURIMetaData>{meta}</CurrentURIMetaData>");
        soap(net, ip, AVT, "SetAVTransportURI", &args, true).await?;
    } else {
        let id = uuid(net, ip).await?;
        soap(net, ip, AVT, "RemoveAllTracksFromQueue", "", true).await?;
        let args = format!(
            "<EnqueuedURI>{uri}</EnqueuedURI><EnqueuedURIMetaData>{meta}</EnqueuedURIMetaData>\
             <DesiredFirstTrackNumberEnqueued>0</DesiredFirstTrackNumberEnqueued><EnqueueAsNext>0</EnqueueAsNext>"
        );
        soap(net, ip, AVT, "AddURIToQueue", &args, true).await?;
        let args = format!("<CurrentURI>x-rincon-queue:{id}#0</CurrentURI><CurrentURIMetaData></CurrentURIMetaData>");
        soap(net, ip, AVT, "SetAVTransportURI", &args, true).await?;
    }
    soap(net, ip, AVT, "Play", "<Speed>1</Speed>", true).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tag;

    #[test]
    fn tag_matches_whole_names() {
        let xml = "<upnp:albumArtURI>/getaa?s=1</upnp:albumArtURI><upnp:album>Moral Panic</upnp:album>";
        assert_eq!(tag(xml, "upnp:album"), "Moral Panic");
        assert_eq!(tag(xml, "upnp:albumArtURI"), "/getaa?s=1");
        assert_eq!(tag("<dc:title id=\"x\">Hi</dc:title>", "dc:title"), "Hi");
        assert_eq!(tag("<a>1</a>", "b"), "");
    }
}
