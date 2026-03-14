use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    pub target: Option<String>,
    pub min_severity: Option<u8>,
    pub fail_on_smell: Option<bool>,
    pub magic_number_whitelist: Option<Vec<i64>>,
    pub assertion_roulette_threshold: Option<usize>,
    pub agent_rules: Option<String>,
    pub llm_command: Option<String>,
    pub agent_confidence: Option<f64>,
    pub glob: Option<String>,
}

impl ProjectConfig {
    /// 指定ディレクトリから .savanna.toml を探す。見つからなければ親を辿る。
    pub fn load(dir: &Path) -> Self {
        // 指定ディレクトリから親を辿って .savanna.toml を探す
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let mut current = Some(dir.as_path());
        while let Some(d) = current {
            let config_path = d.join(".savanna.toml");
            if config_path.exists() {
                return Self::load_from(&config_path);
            }
            current = d.parent();
        }
        Self::default()
    }

    fn load_from(config_path: &Path) -> Self {
        match std::fs::read_to_string(config_path) {
            Ok(content) => {
                match toml::from_str(&content) {
                    Ok(config) => {
                        eprintln!("  Using config from {}", config_path.display());
                        config
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to parse {} — {}", config_path.display(), e);
                        Self::default()
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: failed to read {} — {}", config_path.display(), e);
                Self::default()
            }
        }
    }
}
