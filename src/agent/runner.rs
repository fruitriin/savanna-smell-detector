use std::io::Write;
use std::process::{Command, Stdio};
use crate::core::TestFile;
use super::types::{AgentInput, AgentInputMetadata, AgentOutput, AgentRule, AgentTestSmell};
use super::prefilter::apply_prefilter;

/// Agent検出を実行して結果を返す
pub fn run_agent_detection(
    rules: &[AgentRule],
    test_files: &[TestFile],
    default_llm_command: &str,
    confidence_threshold: f64,
) -> Vec<AgentTestSmell> {
    let mut results = Vec::new();

    for rule in rules {
        let llm_command = rule.llm_command.as_deref().unwrap_or(default_llm_command);

        for test_file in test_files {
            for func in &test_file.test_functions {
                if !apply_prefilter(&rule.prefilter, func) {
                    continue;
                }

                let input = AgentInput {
                    file_path: test_file.path.clone(),
                    function_name: func.name.clone(),
                    line: func.line,
                    language: test_file.language.clone(),
                    body_source: func.body_source.clone(),
                    metadata: AgentInputMetadata {
                        assertion_count: func.assertion_count,
                        has_conditional: func.has_conditional,
                        has_sleep: func.has_sleep,
                        has_print: func.has_print,
                        is_ignored: func.is_ignored,
                    },
                };

                let input_json = match serde_json::to_string(&input) {
                    Ok(j) => j,
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to serialize input for {}::{} — {}",
                            test_file.path, func.name, e
                        );
                        continue;
                    }
                };

                let prompt = rule.prompt_template.replace("{{input}}", &input_json);

                match call_llm(llm_command, &prompt) {
                    Ok(output_str) => {
                        match parse_llm_output(&output_str) {
                            Ok(agent_output) => {
                                if !agent_output.is_smell {
                                    continue;
                                }
                                if agent_output.confidence < confidence_threshold {
                                    continue;
                                }
                                results.push(AgentTestSmell {
                                    rule_name: rule.name.clone(),
                                    description: rule.description.clone(),
                                    severity: rule.severity,
                                    file_path: test_file.path.clone(),
                                    line: func.line,
                                    function_name: func.name.clone(),
                                    confidence: agent_output.confidence,
                                    reason: agent_output.reason,
                                    suggestion: agent_output.suggestion,
                                });
                            }
                            Err(e) => {
                                eprintln!(
                                    "Warning: failed to parse LLM output for rule '{}', {}::{} — {}",
                                    rule.name, test_file.path, func.name, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: LLM call failed for rule '{}', {}::{} — {}",
                            rule.name, test_file.path, func.name, e
                        );
                    }
                }
            }
        }
    }

    results
}

/// LLMコマンドを実行してstdoutを返す
fn call_llm(command_str: &str, prompt: &str) -> Result<String, String> {
    // "claude -p" のようなコマンド文字列を分割
    let parts: Vec<&str> = command_str.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty LLM command".to_string());
    }

    let (cmd, args) = parts.split_first().unwrap();

    let mut child = Command::new(cmd)
        .args(args)
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {}", command_str, e))?;

    // stdinにプロンプトを書き込む
    if let Some(stdin) = child.stdin.take() {
        let mut stdin = stdin;
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for LLM process: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "LLM command exited with status {}: {}",
            output.status, stderr
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| format!("LLM output is not valid UTF-8: {}", e))
}

/// LLMの出力文字列からAgentOutputをパースする
/// JSON部分を抽出してデシリアライズする
fn parse_llm_output(output: &str) -> Result<AgentOutput, String> {
    // まず出力全体をJSONとしてパースを試みる
    if let Ok(parsed) = serde_json::from_str::<AgentOutput>(output.trim()) {
        return Ok(parsed);
    }

    // JSON部分を抽出（```json ... ``` のコードブロックや { ... } を探す）
    let json_str = extract_json(output)
        .ok_or_else(|| format!("No valid JSON found in LLM output: {}", &output[..output.len().min(200)]))?;

    serde_json::from_str::<AgentOutput>(json_str)
        .map_err(|e| format!("Failed to parse AgentOutput JSON: {}", e))
}

/// 文字列からJSON部分を抽出する
fn extract_json(s: &str) -> Option<&str> {
    // ```json ... ``` ブロックを優先して探す
    if let Some(start) = s.find("```json") {
        let after = &s[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim());
        }
    }

    // ``` ... ``` ブロック
    if let Some(start) = s.find("```") {
        let after = &s[start + 3..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.starts_with('{') {
                return Some(candidate);
            }
        }
    }

    // { ... } を探す（最初の { から対応する } まで）
    let start = s.find('{')?;
    let mut depth = 0i32;
    let chars = s[start..].char_indices();
    for (i, c) in chars {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=start + i]);
                }
            }
            _ => {}
        }
    }

    None
}
