//! Settings that survive a restart, kept readable only by their owner.

use serde_json::{Map, Value};
use std::io;
use std::path::{Path, PathBuf};

mod private_file;

#[derive(Clone)]
pub struct Preferences {
    path: PathBuf,
    values: Map<String, Value>,
    save_blocked: bool,
}

impl Preferences {
    /// Falls back to an in-memory store when there is no home directory, so a broken
    /// environment degrades to unsaved settings rather than refusing to start.
    pub fn load() -> Self {
        match directories::ProjectDirs::from("dev", "", "kavverna") {
            Some(dirs) => Self::load_from(dirs.config_dir().join("settings.json")),
            None => {
                tracing::warn!("no config directory, settings will not persist");
                Self { path: PathBuf::new(), values: Map::new(), save_blocked: false }
            }
        }
    }

    pub fn load_from(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (values, save_blocked) = match std::fs::read_to_string(&path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => (Map::new(), false),
            Err(err) => {
                tracing::error!(%err, path = %path.display(), "settings could not be read");
                (Map::new(), true)
            }
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(map)) => (map, false),
                Ok(_) => {
                    tracing::error!(path = %path.display(), "settings are not a JSON object");
                    (Map::new(), true)
                }
                Err(err) => {
                    tracing::error!(%err, path = %path.display(), "settings JSON is invalid");
                    (Map::new(), true)
                }
            },
        };

        Self { path, values, save_blocked }
    }

    pub fn bool(&self, key: &str, fallback: bool) -> bool {
        self.values.get(key).and_then(Value::as_bool).unwrap_or(fallback)
    }

    pub fn integer(&self, key: &str, fallback: i64) -> i64 {
        self.values.get(key).and_then(Value::as_i64).unwrap_or(fallback)
    }

    pub fn text(&self, key: &str, fallback: &str) -> String {
        self.values.get(key).and_then(Value::as_str).unwrap_or(fallback).to_owned()
    }

    /// `None` for a key that was never written, which is not the same as one deliberately left
    /// empty: a chosen set of devices that nobody has chosen from yet means all of them, and a
    /// set somebody emptied means none.
    pub fn texts(&self, key: &str) -> Option<Vec<String>> {
        let items = self.values.get(key)?.as_array()?;
        Some(items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.values.insert(key.to_owned(), Value::Bool(value));
    }

    pub fn set_integer(&mut self, key: &str, value: i64) {
        self.values.insert(key.to_owned(), Value::Number(value.into()));
    }

    pub fn set_text(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_owned(), Value::String(value.to_owned()));
    }

    pub fn set_texts(&mut self, key: &str, values: &[String]) {
        let items = values.iter().map(|value| Value::String(value.clone())).collect();
        self.values.insert(key.to_owned(), Value::Array(items));
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn can_save(&self) -> bool {
        !self.save_blocked && !self.path.as_os_str().is_empty()
    }

    pub fn save(&self) -> io::Result<()> {
        if self.save_blocked {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "saved settings were not readable",
            ));
        }
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }

        let text = serde_json::to_string_pretty(&Value::Object(self.values.clone()))
            .map_err(io::Error::other)?;

        private_file::write(&self.path, text.as_bytes())
    }
}
