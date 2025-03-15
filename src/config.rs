use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub concurrency: u64,
    pub interval: u64, // in seconds
    pub metrics: HashMap<String, MetricConfig>,
}

#[derive(Debug, Deserialize)]
pub struct MetricConfig {
    pub enabled: bool,
    pub adapter: String,
    pub config: serde_json::Value,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
