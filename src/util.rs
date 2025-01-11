use osascript::JavaScript;
use serde::Deserialize;

use crate::{ITunesAppName, MusicError, ScriptParams, MAC_OS_CATALINA};

pub fn get_macos_version() -> f32 {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .expect("Failed to get macOS version");

    let version = String::from_utf8_lossy(&output.stdout);

    let parts: Vec<&str> = version.trim().split('.').collect();

    if parts.len() >= 2 {
        let version_str = format!("{}.{}", parts[0], parts[1]);
        version_str.parse::<f32>().unwrap_or(MAC_OS_CATALINA)
    } else {
        MAC_OS_CATALINA
    }
}

pub fn execute_script<T>(app_name: &ITunesAppName, script: &str) -> Result<T, MusicError>
where
    T: for<'de> Deserialize<'de>,
{
    let command = JavaScript::new(script);
    let params = ScriptParams {
        name: app_name.to_string(),
    };

    command
        .execute_with_params(params)
        .map_err(MusicError::from)
}
