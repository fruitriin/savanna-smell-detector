use serde::{Deserialize, Serialize};

/// Agent検出器のプレフィルター設定
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PrefilterConfig {
    pub min_assertions: Option<usize>,
    pub max_assertions: Option<usize>,
    pub has_conditional: Option<bool>,
    pub has_sleep: Option<bool>,
    pub min_lines: Option<usize>,
}

/// Agent検出器のルール定義
#[derive(Debug, Clone)]
pub struct AgentRule {
    pub name: String,
    pub description: String,
    pub severity: u8,
    pub prefilter: PrefilterConfig,
    pub llm_command: Option<String>,
    pub prompt_template: String,
}

/// LLMに送る入力
#[derive(Debug, Serialize)]
pub struct AgentInput {
    pub file_path: String,
    pub function_name: String,
    pub line: usize,
    pub language: String,
    pub body_source: String,
    pub metadata: AgentInputMetadata,
}

/// LLMに送るメタデータ
#[derive(Debug, Serialize)]
pub struct AgentInputMetadata {
    pub assertion_count: usize,
    pub has_conditional: bool,
    pub has_sleep: bool,
    pub has_print: bool,
    pub is_ignored: bool,
}

/// LLMの出力
#[derive(Debug, Deserialize)]
pub struct AgentOutput {
    pub is_smell: bool,
    pub confidence: f64,
    pub reason: String,
    pub suggestion: Option<String>,
}

/// Agent検出器の結果（TestSmellとは別の型）
#[derive(Debug, Clone, Serialize)]
pub struct AgentTestSmell {
    pub rule_name: String,
    pub description: String,
    pub severity: u8,
    pub file_path: String,
    pub line: usize,
    pub function_name: String,
    pub confidence: f64,
    pub reason: String,
    pub suggestion: Option<String>,
}
