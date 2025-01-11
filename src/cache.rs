use crate::structs::ITunesInfos;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Failed to read cache file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Failed to parse cache data: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Cache version mismatch")]
    VersionMismatch,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    data: ITunesInfos,
    #[serde(with = "timestamp_serde")]
    created_at: SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cache {
    version: i32,
    #[serde(skip)]
    cache_file: PathBuf,
    data: HashMap<String, CacheEntry>,
    #[serde(skip)]
    max_age: Duration,
    #[serde(skip)]
    dirty: bool,
}

mod timestamp_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

fn cache_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .map(PathBuf::from)
        .map(|h| h.join("Library/Caches"))
}

impl Cache {
    pub fn new() -> Self {
        let cache_path = cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("apple-music-rpc.cache");

        Self {
            version: 3,
            cache_file: cache_path,
            data: HashMap::new(),
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 1 week default
            dirty: false,
        }
    }

    pub fn get(&mut self, key: String) -> Option<&ITunesInfos> {
        self.cleanup_expired();
        self.data.get(&key).map(|entry| &entry.data)
    }

    pub fn set(&mut self, key: String, value: ITunesInfos) {
        let entry = CacheEntry {
            data: value,
            created_at: SystemTime::now(),
        };
        self.data.insert(key, entry);
        self.dirty = true;

        if self.data.len() > 1000 {
            self.cleanup_expired();
        }
    }

    fn cleanup_expired(&mut self) {
        let now = SystemTime::now();
        self.data.retain(|_, entry| {
            now.duration_since(entry.created_at)
                .map(|age| age <= self.max_age)
                .unwrap_or(false)
        });
    }

    pub fn load_cache(&mut self) -> Result<(), CacheError> {
        match fs::read_to_string(&self.cache_file) {
            Ok(text) => {
                #[derive(Deserialize)]
                struct CacheData {
                    version: i32,
                    data: HashMap<String, CacheEntry>,
                }

                let cache_data: CacheData = serde_json::from_str(&text)?;
                if cache_data.version != self.version {
                    return Err(CacheError::VersionMismatch);
                }
                self.data = cache_data.data;
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CacheError::FileReadError(e)),
        }
    }

    pub fn save_cache(&mut self) -> Result<(), CacheError> {
        if !self.dirty {
            return Ok(());
        }

        self.cleanup_expired();

        if let Some(parent) = self.cache_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let text = serde_json::to_string_pretty(&self)?;
        fs::write(&self.cache_file, text)?;
        self.dirty = false;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), CacheError> {
        self.save_cache()
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.save_cache();
        }
    }
}
