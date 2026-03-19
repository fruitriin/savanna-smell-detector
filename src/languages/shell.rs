use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use regex::Regex;

pub struct ShellParser;

impl LanguageParser for ShellParser {
    fn extensions(&self) -> &[&str] {
        &["sh", "bash", "bats"]
    }

    fn language_name(&self) -> &str {
        "shell"
    }

    fn parse(&self, path: &str, source: &str) -> Option<TestFile> {
        let test_functions = extract_test_functions(source);

        Some(TestFile {
            path: path.to_string(),
            language: "shell".to_string(),
            test_functions,
            source: Some(source.to_string()),
        })
    }
}

/// テスト関数を抽出する
fn extract_test_functions(source: &str) -> Vec<TestFunction> {
    let lines: Vec<&str> = source.lines().collect();
    let mut functions = Vec::new();

    // Bats: @test "name" {
    let bats_re = Regex::new(r#"^\s*@test\s+"([^"]+)"\s*\{"#).unwrap();
    // shunit2 / plain: test_xxx() { or testXxx() {
    let func_re = Regex::new(r#"^\s*(test[A-Za-z0-9_]*)\s*\(\s*\)\s*\{"#).unwrap();
    // function test_xxx { (alternative syntax)
    let func_kw_re = Regex::new(r#"^\s*function\s+(test[A-Za-z0-9_]*)\s*(?:\(\s*\))?\s*\{"#).unwrap();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        let (name, start_line) = if let Some(caps) = bats_re.captures(line) {
            (caps[1].to_string(), i)
        } else if let Some(caps) = func_re.captures(line) {
            (caps[1].to_string(), i)
        } else if let Some(caps) = func_kw_re.captures(line) {
            (caps[1].to_string(), i)
        } else {
            i += 1;
            continue;
        };

        // 波括弧を追跡して関数ボディを抽出
        if let Some((body, end_line)) = extract_body(&lines, i) {
            let test_fn = analyze_shell_function(&name, start_line + 1, &body, &lines[i..=end_line]);
            functions.push(test_fn);
            i = end_line + 1;
        } else {
            i += 1;
        }
    }

    functions
}

/// 波括弧のネストを追跡して関数ボディを取得する
/// 開始行（`{` を含む行）から閉じ `}` までを返す
fn extract_body(lines: &[&str], start: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    let mut body_lines = Vec::new();
    let mut in_single_quote = false;
    let mut in_heredoc: Option<String> = None;

    for i in start..lines.len() {
        let line = lines[i];

        // ヒアドキュメント内の処理
        if let Some(ref delimiter) = in_heredoc {
            if line.trim() == delimiter.as_str() {
                in_heredoc = None;
            }
            if i > start {
                body_lines.push(line);
            }
            continue;
        }

        // ヒアドキュメントの開始を検出
        if let Some(heredoc_delim) = detect_heredoc(line) {
            in_heredoc = Some(heredoc_delim);
        }

        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            let ch = chars[j];

            // シングルクォート内ではすべてリテラル
            if in_single_quote {
                if ch == '\'' {
                    in_single_quote = false;
                }
                j += 1;
                continue;
            }

            match ch {
                '\'' => {
                    in_single_quote = true;
                }
                '#' => {
                    // コメントの残り部分はスキップ（クォート外のみ）
                    break;
                }
                '"' => {
                    // ダブルクォート内をスキップ（エスケープを考慮）
                    j += 1;
                    while j < chars.len() {
                        if chars[j] == '\\' {
                            j += 2; // エスケープをスキップ
                            continue;
                        }
                        if chars[j] == '"' {
                            break;
                        }
                        j += 1;
                    }
                }
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        // ボディは開始行の { 以降〜閉じ } の前まで
                        if i > start {
                            body_lines.push(line);
                        }
                        let body = if i == start {
                            // 一行関数
                            extract_inline_body(lines[start])
                        } else {
                            // 最初の行（関数宣言行）と最後の行（閉じ}行）を除いたボディ
                            let inner: Vec<&str> = body_lines.iter()
                                .take(body_lines.len().saturating_sub(1)) // 閉じ } の行を除く
                                .copied()
                                .collect();
                            inner.join("\n")
                        };
                        return Some((body, i));
                    }
                }
                _ => {}
            }
            j += 1;
        }

        if i > start {
            body_lines.push(line);
        }
    }

    None
}

/// ヒアドキュメントの開始を検出し、デリミタを返す
fn detect_heredoc(line: &str) -> Option<String> {
    let heredoc_re = Regex::new(r#"<<-?\s*'?([A-Za-z_][A-Za-z0-9_]*)'?"#).unwrap();
    heredoc_re.captures(line).map(|caps| caps[1].to_string())
}

/// 一行関数からボディを抽出: `test_foo() { echo "hi"; }` → `echo "hi";`
fn extract_inline_body(line: &str) -> String {
    if let Some(start) = line.find('{') {
        let after_brace = &line[start + 1..];
        if let Some(end) = after_brace.rfind('}') {
            return after_brace[..end].trim().to_string();
        }
    }
    String::new()
}

/// Shell テスト関数を解析して TestFunction を生成
fn analyze_shell_function(name: &str, line: usize, body: &str, _raw_lines: &[&str]) -> TestFunction {
    let body_line_count = body.lines().count();

    // 空テスト: ボディが空白・コメントのみ
    let is_empty = body.lines().all(|l| {
        let trimmed = l.trim();
        trimmed.is_empty() || trimmed.starts_with('#')
    });

    // skip / startSkipping → ignored
    let is_ignored = has_pattern(body, &["skip", "startSkipping"]);

    // アサーション検出
    let assertion_patterns = [
        // Bats assertions
        "assert", "assert_success", "assert_failure", "assert_output",
        "assert_line", "assert_equal", "refute",
        // shunit2 assertions
        "assertEquals", "assertNotEquals", "assertSame", "assertNotSame",
        "assertNull", "assertNotNull", "assertTrue", "assertFalse",
        "assertContains",
        // fail
        "fail",
    ];
    let assertion_count = count_assertions(body, &assertion_patterns);
    // [ ... ] and [[ ... ]] test commands
    let bracket_assertion_count = count_bracket_assertions(body);
    let total_assertion_count = assertion_count + bracket_assertion_count;
    let has_assertion = total_assertion_count > 0 || has_exit_code_check(body);

    // sleep
    let has_sleep = has_command(body, "sleep");

    // 条件分岐
    let has_if = body.lines().any(|l| {
        let t = l.trim();
        t.starts_with("if ") || t.starts_with("if[") || t == "if"
    });
    let has_case = body.lines().any(|l| {
        let t = l.trim();
        t.starts_with("case ")
    });
    let has_for = body.lines().any(|l| {
        let t = l.trim();
        t.starts_with("for ")
    });
    let has_while = body.lines().any(|l| {
        let t = l.trim();
        t.starts_with("while ")
    });
    let has_conditional = has_if || has_case || has_for || has_while;
    let has_branching = has_if || has_case;

    // print 検出 (echo/printf — ただしアサーションメッセージとしての echo "FAIL" 等は除外しにくいため全検出)
    let has_print = has_command(body, "echo") || has_command(body, "printf");

    // early return (先頭3行以内の return/exit)
    let has_early_return = body.lines().take(3).any(|l| {
        let t = l.trim();
        t.starts_with("return") || t.starts_with("exit")
    });

    // timeout コマンド
    let has_timeout_dependency = has_command(body, "timeout");

    // magic numbers (アサーション行内の大きな数値)
    let magic_numbers = extract_magic_numbers(body);

    // for ループ内のアサーション（簡易判定）
    let has_assertion_in_loop = check_assertion_in_loop(body);

    TestFunction {
        name: name.to_string(),
        line,
        body_source: body.to_string(),
        is_ignored,
        has_assertion,
        has_sleep,
        has_conditional,
        has_branching,
        has_for_loop: has_for,
        has_assertion_in_loop,
        has_print,
        is_empty,
        assertion_count: total_assertion_count,
        assert_only_count: bracket_assertion_count,
        assertions_without_message: 0, // Shell では簡易的に 0
        assert_only_without_message: 0,
        magic_numbers,
        has_early_return,
        has_timeout_dependency,
        body_line_count,
    }
}

/// ボディ内に指定パターン（コマンド/キーワード）があるか
fn has_pattern(body: &str, patterns: &[&str]) -> bool {
    body.lines().any(|l| {
        let t = l.trim();
        // コメント行はスキップ
        if t.starts_with('#') {
            return false;
        }
        patterns.iter().any(|p| {
            // 単語境界を考慮: コマンドとして出現するか
            t == *p || t.starts_with(&format!("{} ", p)) || t.contains(&format!(" {} ", p))
                || t.contains(&format!(" {}", p)) || t.starts_with(&format!("{}(", p))
        })
    })
}

/// コマンドが使われているか（コメント行は除外）
fn has_command(body: &str, cmd: &str) -> bool {
    let re = Regex::new(&format!(r"\b{}\b", regex::escape(cmd))).unwrap();
    body.lines().any(|l| {
        let t = l.trim();
        if t.starts_with('#') {
            return false;
        }
        re.is_match(t)
    })
}

/// アサーションコマンドの出現回数をカウント
fn count_assertions(body: &str, patterns: &[&str]) -> usize {
    let mut count = 0;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        for pattern in patterns {
            let re = Regex::new(&format!(r"\b{}\b", regex::escape(pattern))).unwrap();
            if re.is_match(t) {
                count += 1;
                break; // 1行につき1カウント
            }
        }
    }
    count
}

/// [ ... ] と [[ ... ]] によるテストコマンドのカウント
fn count_bracket_assertions(body: &str) -> usize {
    let bracket_re = Regex::new(r"\[\[?\s+.+\s+\]\]?").unwrap();
    body.lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with('#') && bracket_re.is_match(t)
        })
        .count()
}

/// exit code チェック ($? の参照) があるか
fn has_exit_code_check(body: &str) -> bool {
    body.contains("$?")
}

/// アサーション行から大きなマジックナンバーを抽出
fn extract_magic_numbers(body: &str) -> Vec<(i64, usize)> {
    let num_re = Regex::new(r"\b(\d+)\b").unwrap();
    let mut result = Vec::new();

    for (i, line) in body.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        // アサーション系の行のみ
        if t.contains("assert") || t.contains("assertEquals") || t.contains("[ ") || t.contains("[[ ") {
            for caps in num_re.captures_iter(t) {
                if let Ok(n) = caps[1].parse::<i64>() {
                    if !(-10..=10).contains(&n) {
                        result.push((n, i + 1));
                    }
                }
            }
        }
    }
    result
}

/// for ループ内にアサーションがあるかの簡易チェック
fn check_assertion_in_loop(body: &str) -> bool {
    let mut in_for = false;
    let mut depth = 0;

    for line in body.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }

        if t.starts_with("for ") {
            in_for = true;
            depth = 0;
        }

        if in_for {
            depth += t.matches('{').count() as i32;
            depth -= t.matches('}').count() as i32;

            // アサーションがあるか
            if t.contains("assert") || t.contains("assertEquals")
                || t.contains("[ ") || t.contains("[[ ")
            {
                return true;
            }

            if t == "done" || (depth <= 0 && t.contains('}')) {
                in_for = false;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bats_function_extraction() {
        let source = r#"
@test "addition works" {
    result=$((2 + 2))
    [ "$result" -eq 4 ]
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "addition works");
        assert!(fns[0].has_assertion);
        assert!(!fns[0].is_empty);
    }

    #[test]
    fn test_shunit2_function_extraction() {
        let source = r#"
test_addition() {
    result=$((2 + 2))
    assertEquals 4 "$result"
}

testSubtraction() {
    result=$((5 - 3))
    assertEquals 2 "$result"
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "test_addition");
        assert_eq!(fns[1].name, "testSubtraction");
        assert!(fns[0].has_assertion);
        assert!(fns[1].has_assertion);
    }

    #[test]
    fn test_function_keyword_syntax() {
        let source = r#"
function test_something {
    echo "testing"
    [ 1 -eq 1 ]
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "test_something");
        assert!(fns[0].has_print);
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_empty_test_detection() {
        let source = r#"
@test "empty test" {
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_empty);
    }

    #[test]
    fn test_missing_assertion() {
        let source = r#"
test_no_assert() {
    result=$(some_command)
    echo "$result"
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(!fns[0].has_assertion);
        assert!(fns[0].has_print);
    }

    #[test]
    fn test_sleepy_test() {
        let source = r#"
@test "sleepy" {
    sleep 2
    [ "$status" -eq 0 ]
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_sleep);
    }

    #[test]
    fn test_conditional_logic() {
        let source = r#"
test_with_if() {
    if [ -f /tmp/test ]; then
        assertEquals "exists" "exists"
    fi
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_conditional);
        assert!(fns[0].has_branching);
    }

    #[test]
    fn test_ignored_with_skip() {
        let source = r#"
@test "skipped test" {
    skip "not ready yet"
    [ 1 -eq 1 ]
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_ignored);
    }

    #[test]
    fn test_exit_code_assertion() {
        let source = r#"
test_exit_code() {
    some_command
    if [ $? -ne 0 ]; then
        fail "command failed"
    fi
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion); // $? check counts as assertion
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"#!/bin/bash

@test "first" {
    [ 1 -eq 1 ]
}

@test "second" {
    [ 2 -eq 2 ]
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].line, 3); // 1-indexed
        assert_eq!(fns[1].line, 7);
    }

    #[test]
    fn test_nested_braces() {
        let source = r#"
test_nested() {
    if [ 1 -eq 1 ]; then
        for i in 1 2 3; do
            echo "$i"
        done
    fi
    [ "$?" -eq 0 ]
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion);
        assert!(fns[0].has_conditional);
    }

    #[test]
    fn test_comment_only_body() {
        let source = r#"
test_comments_only() {
    # TODO: implement this test
    # another comment
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_empty); // comments-only is empty
    }
}
