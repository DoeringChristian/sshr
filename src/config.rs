use std::fs;
use std::path::PathBuf;

pub struct Config {
    pub shell: Option<String>,
}

impl Config {
    pub fn load() -> Config {
        let path = config_path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Config { shell: None },
        };

        let mut shell = None;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if key == "shell" {
                    shell = Some(value.to_string());
                }
            }
        }

        Config { shell }
    }
}

fn config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir).join("sshr/config")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/sshr/config")
    } else {
        PathBuf::from("~/.config/sshr/config")
    }
}
