use anyhow::{Context, Result};
use serde::Deserialize;

const DEFAULT_CONFIG_LOC: &str = "./oncmp_config.toml";

#[derive(Debug, Deserialize)]
struct RawConfig {
    run: RunConfig,
    #[serde(default)]
    ignore: IgnoreConfig,
}

#[derive(Debug, Deserialize)]
struct RunConfig {
    old_repo: String,
    new_repo: String,
    model_file: String,
}

#[derive(Debug, Default, Deserialize)]
struct IgnoreConfig {
    #[serde(default)]
    params: Vec<String>,
    #[serde(default)]
    tests: Vec<String>,
}

#[derive(Debug)]
pub struct Config {
    pub ignore_params: Vec<String>,
    pub ignore_tests: Vec<String>,
    pub old_repo: String,
    pub new_repo: String,
    pub model_file: String,
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self {
        Self {
            ignore_params: raw.ignore.params,
            ignore_tests: raw.ignore.tests,
            old_repo: raw.run.old_repo,
            new_repo: raw.run.new_repo,
            model_file: raw.run.model_file,
        }
    }
}

pub fn load(config_loc: Option<&str>) -> Result<Config> {
    let path = config_loc.unwrap_or(DEFAULT_CONFIG_LOC);

    let contents =
        std::fs::read_to_string(path).with_context(|| format!("failed to read config: {path}"))?;

    let raw: RawConfig =
        toml::from_str(&contents).with_context(|| format!("failed to parse config: {path}"))?;

    let config = Config::from(raw);
    validate(&config)?;

    Ok(config)
}

fn validate(config: &Config) -> Result<()> {
    anyhow::ensure!(
        std::path::Path::new(&config.old_repo).is_dir(),
        "old_repo directory does not exist: {:?}",
        config.old_repo
    );

    anyhow::ensure!(
        std::path::Path::new(&config.new_repo).is_dir(),
        "new_repo directory does not exist: {:?}",
        config.new_repo
    );

    anyhow::ensure!(
        !config.model_file.trim().is_empty(),
        "model_file must not be empty"
    );
    Ok(())
}
