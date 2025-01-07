use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ITunesAppName {
    Music,
    ITunes,
}

impl ToString for ITunesAppName {
    fn to_string(&self) -> String {
        match self {
            ITunesAppName::Music => "Music".to_string(),
            ITunesAppName::ITunes => "iTunes".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ITunesProps {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ITunesInfos {
    pub artwork: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseOuter {
    pub results: Vec<ResponseInner>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseInner {
    #[serde(rename = "artworkUrl100")]
    pub artwork_url_100: Option<String>,
    #[serde(rename = "artworkUrl600")]
    pub artwork_url_600: Option<String>,
    #[serde(rename = "collectionViewUrl")]
    pub collection_view_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CacheError(pub String);

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cache error: {}", self.0)
    }
}

impl Error for CacheError {}

#[derive(Debug)]
pub enum MusicError {
    ScriptError(osascript::Error),
    SystemError(String),
    NetworkError(reqwest::Error),
    SerializationError(serde_json::Error),
    UrlParseError(url::ParseError),
    CacheError(String),
    DiscordError(String),
}

impl fmt::Display for MusicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MusicError::ScriptError(e) => write!(f, "AppleScript error: {}", e),
            MusicError::SystemError(e) => write!(f, "System error: {}", e),
            MusicError::NetworkError(e) => write!(f, "Network error: {}", e),
            MusicError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            MusicError::UrlParseError(e) => write!(f, "URL parse error: {}", e),
            MusicError::CacheError(e) => write!(f, "Cache error: {}", e),
            MusicError::DiscordError(e) => write!(f, "Discord error: {}", e),
        }
    }
}

impl Error for MusicError {}

impl From<osascript::Error> for MusicError {
    fn from(err: osascript::Error) -> Self {
        MusicError::ScriptError(err)
    }
}

impl From<std::io::Error> for MusicError {
    fn from(err: std::io::Error) -> Self {
        MusicError::SystemError(err.to_string())
    }
}

impl From<reqwest::Error> for MusicError {
    fn from(err: reqwest::Error) -> Self {
        MusicError::NetworkError(err)
    }
}

impl From<serde_json::Error> for MusicError {
    fn from(err: serde_json::Error) -> Self {
        MusicError::SerializationError(err)
    }
}

impl From<url::ParseError> for MusicError {
    fn from(err: url::ParseError) -> Self {
        MusicError::UrlParseError(err)
    }
}

impl From<CacheError> for MusicError {
    fn from(err: CacheError) -> Self {
        MusicError::CacheError(err.0)
    }
}

#[derive(Debug, Clone)]
pub struct PresenceData {
    pub name: String,
    pub artist: String,
    #[allow(dead_code)]
    pub album: String,
    pub artwork_url: Option<String>,
    pub share_url: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
}

impl PresenceData {
    pub fn new(props: &ITunesProps) -> Self {
        Self {
            name: props.name.clone(),
            artist: props.artist.clone(),
            album: props.album.clone(),
            artwork_url: None,
            share_url: None,
            start: None,
            end: None,
        }
    }

    pub fn set_timing(&mut self, start: i64, end: i64) {
        self.start = Some(start);
        self.end = Some(end);
    }

    pub fn set_artwork_info(&mut self, infos: ITunesInfos) {
        self.artwork_url = infos.artwork;
        self.share_url = infos.url;
    }
}

#[derive(Debug)]
pub enum AppState {
    Idle,
    Presence(PresenceData),
}

#[derive(Serialize)]
pub struct ScriptParams {
    pub name: String,
}

pub struct ScriptCollection {
    pub is_open: &'static str,
    pub get_props: &'static str,
    pub get_position: &'static str,
    pub get_state: &'static str,
}

pub const SCRIPTS: ScriptCollection = ScriptCollection {
    is_open: "return Application(\"System Events\").processes[$params.name].exists();",
    get_props: r#"
        var App = Application($params.name);
        return App.currentTrack().properties();
    "#,
    get_position: r#"
        var App = Application($params.name);
        return App.playerPosition();
    "#,
    get_state: r#"
        var App = Application($params.name);
        return App.playerState();
    "#,
};

pub const MAC_OS_CATALINA: f32 = 10.15;
pub const DISCORD_CLIENT_ID: &str = "1326053171809747006";
