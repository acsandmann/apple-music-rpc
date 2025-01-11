use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::{
    signal::unix::{signal, SignalKind},
    time::Duration,
};
use url::Url;

mod cache;
mod structs;
mod util;

use cache::Cache;
use structs::*;
use util::*;

struct App {
    state: AppState,
    client: DiscordIpcClient,
    cache: Cache,
    app_name: ITunesAppName,
}

impl App {
    pub fn new(client_id: &str, app_name: ITunesAppName) -> Result<Self, MusicError> {
        let mut cache = Cache::new();
        let _ = cache.load_cache();

        let client = DiscordIpcClient::new(client_id)
            .map_err(|e| MusicError::DiscordError(e.to_string()))?;

        Ok(App {
            state: AppState::Idle,
            client,
            cache,
            app_name,
        })
    }

    fn try_reconnect(&mut self, client_id: &str) -> bool {
        match DiscordIpcClient::new(client_id) {
            Ok(mut new_client) => match new_client.connect() {
                Ok(()) => {
                    println!("Successfully reconnected to Discord!");
                    self.client = new_client;
                    true
                }
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    async fn search_album_artwork(
        &mut self,
        props: &ITunesProps,
        album: bool,
    ) -> Result<Option<ITunesInfos>, MusicError> {
        let query = format!("{} {}", props.artist, props.name);

        if let Some(infos) = self.cache.get(query.clone()) {
            return Ok(Some(infos.to_owned()));
        }

        let params = if album {
            vec![
                ("media", "music"),
                ("limit", "1"),
                ("term", &query),
                ("entity", "album"),
            ]
        } else {
            vec![("media", "music"), ("limit", "1"), ("term", &query)]
        };

        let url = Url::parse_with_params("https://itunes.apple.com/search?", &params)?;
        let resp: ResponseOuter = reqwest::get(url.as_str()).await?.json().await?;

        if resp.results.is_empty() {
            if album {
                return Box::pin(self.search_album_artwork(props, false)).await;
            } else {
                return Ok(None);
            }
        }

        let res = &resp.results[0];
        let artwork = if res.artwork_url_600.is_some() {
            res.artwork_url_600.clone()
        } else {
            res.artwork_url_100.clone()
        };

        let infos = ITunesInfos {
            artwork: artwork,
            url: res.collection_view_url.clone(),
        };

        self.cache.set(query, infos.clone());
        Ok(Some(infos))
    }

    async fn update_presence(&mut self) -> Result<AppState, MusicError> {
        let state: String = execute_script(&self.app_name, SCRIPTS.get_state)?;

        if state != "playing" {
            return Ok(AppState::Idle);
        }

        let props: ITunesProps = execute_script(&self.app_name, SCRIPTS.get_props)?;
        let mut presence_data = PresenceData::new(&props);

        if let Some(duration) = props.duration {
            let player_position: f64 = execute_script(&self.app_name, SCRIPTS.get_position)?;
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards");

            let start = current_time.as_secs() as i64 * 1000 - (player_position * 1000.0) as i64;
            let end = start + (duration * 1000.0) as i64;

            presence_data.set_timing(start, end);
        }

        if !props.album.is_empty() {
            if let Ok(Some(infos)) = self.search_album_artwork(&props, true).await {
                presence_data.set_artwork_info(infos);
            }
        }

        Ok(AppState::Presence(presence_data))
    }

    async fn handle_state(&mut self) -> Result<bool, MusicError> {
        let is_open: bool = execute_script(&self.app_name, SCRIPTS.is_open)?;

        if !is_open {
            if let Err(e) = self.client.clear_activity() {
                eprintln!("Failed to clear activity: {}", e);
                return Ok(false);
            }
            self.state = AppState::Idle;
            return Ok(true);
        }

        match self.update_presence().await? {
            AppState::Idle => {
                if let Err(e) = self.client.clear_activity() {
                    eprintln!("Failed to clear activity: {}", e);
                    return Ok(false);
                }
                self.state = AppState::Idle;
                Ok(true)
            }
            AppState::Presence(data) => {
                let mut activity_builder = activity::Activity::new()
                    .details(&data.name)
                    .activity_type(activity::ActivityType::Listening);

                if !data.artist.is_empty() {
                    activity_builder = activity_builder.state(&data.artist);
                }

                if let (Some(start), Some(end)) = (data.start, data.end) {
                    activity_builder = activity_builder
                        .timestamps(activity::Timestamps::new().start(start).end(end));
                }

                let artwork = data
                    .artwork_url
                    .clone()
                    .unwrap_or_else(|| "appicon".to_string());
                let assets = activity::Assets::new().large_image(&artwork);
                activity_builder = activity_builder.assets(assets);

                if let Some(url) = &data.share_url {
                    activity_builder = activity_builder
                        .buttons(vec![activity::Button::new("Listen on Apple Music", url)]);
                }

                match self.client.set_activity(activity_builder) {
                    Ok(_) => {
                        self.state = AppState::Presence(data);
                        Ok(true)
                    }
                    Err(e) => {
                        eprintln!("Failed to set activity: {}", e);
                        Ok(false)
                    }
                }
            }
        }
    }

    pub async fn run(&mut self) -> Result<(), MusicError> {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        let mut sigint = signal(SignalKind::interrupt())?;
        let mut sigterm = signal(SignalKind::terminate())?;

        tokio::spawn(async move {
            tokio::select! {
                _ = sigint.recv() => {
                    println!("\nReceived SIGINT, shutting down...");
                    r.store(false, Ordering::SeqCst);
                }
                _ = sigterm.recv() => {
                    println!("Received SIGTERM, shutting down...");
                    r.store(false, Ordering::SeqCst);
                }
            }
        });

        let client_id = DISCORD_CLIENT_ID.to_string();
        let mut connected = false;

        while running.load(Ordering::SeqCst) {
            if !connected {
                if self.try_reconnect(&client_id) {
                    connected = true;
                    println!("Connected to Discord!");
                } else {
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    continue;
                }
            }

            match self.handle_state().await {
                Ok(true) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Ok(false) => {
                    println!(
                        "Lost connection to Discord, attempting to reconnect in 15 seconds..."
                    );
                    connected = false;
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        if let Err(e) = self.cache.flush() {
            eprintln!("Failed to flush cache: {}", e);
        } else {
            println!("Cache flushed, shutting down gracefully.");
        };

        println!("Cache flushed, shutting down gracefully.");

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), MusicError> {
    let app_name = if get_macos_version() >= MAC_OS_CATALINA {
        ITunesAppName::Music
    } else {
        ITunesAppName::ITunes
    };

    let mut app = App::new(DISCORD_CLIENT_ID, app_name)?;
    app.run().await
}
