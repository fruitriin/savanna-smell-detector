use std::path::Path;
use serde::Deserialize;
use super::types::{AgentRule, PrefilterConfig};

/// YAMLフロントマターの中身
#[derive(Debug, Deserialize)]
struct RuleFrontmatter {
    name: String,
    description: String,
    severity: u8,
    #[serde(default)]
    prefilter: PrefilterConfig,
    llm_command: Option<String>,
}

/// markdownファイルを読み込み、AgentRuleに変換する
pub fn load_rules(dir: &Path) -> Result<Vec<AgentRule>, String> {
    if !dir.exists() {
        return Err(format!("Agent rules directory not found: {}", dir.display()));
    }
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", dir.display()));
    }

    let mut rules = Vec::new();

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    let mut paths: Vec<_> = read_dir
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|ext| ext == "md").unwrap_or(false))
        .collect();
    paths.sort();

    for path in paths {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        match parse_rule(&content) {
            Ok(rule) => rules.push(rule),
            Err(e) => eprintln!("Warning: skipping {} — {}", path.display(), e),
        }
    }

    Ok(rules)
}

/// markdownの文字列をパースしてAgentRuleに変換する
pub fn parse_rule(content: &str) -> Result<AgentRule, String> {
    // YAMLフロントマター（---..---）を分離
    let content = content.trim();

    if !content.starts_with("---") {
        return Err("No YAML frontmatter found (expected '---' at start)".to_string());
    }

    // 最初の "---" の後に続くYAMLを探す
    let after_first = &content[3..];
    let end_idx = after_first
        .find("\n---")
        .ok_or("YAML frontmatter closing '---' not found")?;

    let yaml_str = &after_first[..end_idx];
    let prompt_start = end_idx + 4; // "\n---" の長さ
    let prompt_template = after_first[prompt_start..].trim().to_string();

    if prompt_template.is_empty() {
        return Err("Prompt template is empty (content after frontmatter is required)".to_string());
    }

    let frontmatter: RuleFrontmatter = serde_yaml::from_str(yaml_str)
        .map_err(|e| format!("Failed to parse YAML frontmatter: {}", e))?;

    Ok(AgentRule {
        name: frontmatter.name,
        description: frontmatter.description,
        severity: frontmatter.severity,
        prefilter: frontmatter.prefilter,
        llm_command: frontmatter.llm_command,
        prompt_template,
    })
}
