use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use regex::Regex;

pub struct GoParser;

impl LanguageParser for GoParser {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn language_name(&self) -> &str {
        "go"
    }

    fn parse(&self, path: &str, source: &str) -> Option<TestFile> {
        // Go のテストファイルは *_test.go のみ
        if !path.ends_with("_test.go") {
            return None;
        }

        let test_functions = extract_test_functions(source);

        Some(TestFile {
            path: path.to_string(),
            language: "go".to_string(),
            test_functions,
            source: Some(source.to_string()),
        })
    }
}

/// Go テスト関数を抽出する
fn extract_test_functions(source: &str) -> Vec<TestFunction> {
    let lines: Vec<&str> = source.lines().collect();
    let mut functions = Vec::new();

    // func TestXxx(t *testing.T) {
    // func BenchmarkXxx(b *testing.B) {
    // func FuzzXxx(f *testing.F) {
    // func ExampleXxx() {
    let func_re = Regex::new(
        r#"^\s*func\s+((?:Test|Benchmark|Fuzz|Example)[A-Za-z0-9_]*)\s*\([^)]*\)\s*\{"#
    ).unwrap();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        if let Some(caps) = func_re.captures(line) {
            let name = caps[1].to_string();
            let start_line = i;

            // 波括弧を追跡して関数ボディを抽出
            if let Some((body, end_line)) = extract_body(&lines, i) {
                let test_fn = analyze_go_function(&name, start_line + 1, &body);
                functions.push(test_fn);
                i = end_line + 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    functions
}

/// 波括弧のネストを追跡して関数ボディを取得する
fn extract_body(lines: &[&str], start: usize) -> Option<(String, usize)> {
    let mut depth = 0;
    let mut body_lines = Vec::new();

    for i in start..lines.len() {
        let line = lines[i];
        let mut in_string = false;
        let mut in_rune = false;
        let mut in_raw_string = false;
        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;

        while j < chars.len() {
            let ch = chars[j];

            // バッククォート（raw string literal）内
            if in_raw_string {
                if ch == '`' {
                    in_raw_string = false;
                }
                j += 1;
                continue;
            }

            // ダブルクォート文字列内
            if in_string {
                if ch == '\\' {
                    j += 2;
                    continue;
                }
                if ch == '"' {
                    in_string = false;
                }
                j += 1;
                continue;
            }

            // ルーンリテラル内
            if in_rune {
                if ch == '\\' {
                    j += 2;
                    continue;
                }
                if ch == '\'' {
                    in_rune = false;
                }
                j += 1;
                continue;
            }

            match ch {
                '"' => in_string = true,
                '\'' => in_rune = true,
                '`' => in_raw_string = true,
                '/' if j + 1 < chars.len() && chars[j + 1] == '/' => {
                    // 行コメント — 残りをスキップ
                    break;
                }
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        if i > start {
                            body_lines.push(line);
                        }
                        // 最初の行（関数宣言）と最後の行（閉じ}）を除いたボディ
                        let inner: Vec<&str> = if i == start {
                            // 一行関数
                            vec![]
                        } else {
                            body_lines.iter()
                                .take(body_lines.len().saturating_sub(1))
                                .copied()
                                .collect()
                        };
                        return Some((inner.join("\n"), i));
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

/// Go テスト関数を解析して TestFunction を生成
fn analyze_go_function(name: &str, line: usize, body: &str) -> TestFunction {
    let body_line_count = body.lines().count();

    // 空テスト: ボディが空白・コメントのみ
    let is_empty = body.lines().all(|l| {
        let trimmed = l.trim();
        trimmed.is_empty() || trimmed.starts_with("//")
    });

    // ignored: t.Skip / t.SkipNow / t.Skipf
    let is_ignored = has_pattern(body, &["t.Skip(", "t.SkipNow(", "t.Skipf(", "b.Skip(", "b.SkipNow("]);

    // アサーション検出
    // 標準ライブラリ: t.Error, t.Errorf, t.Fatal, t.Fatalf, t.Fail(), t.FailNow()
    // testify: assert.*, require.*
    let std_assertion_patterns = [
        "t.Error(", "t.Errorf(", "t.Fatal(", "t.Fatalf(",
        "t.Fail(", "t.FailNow(",
        "b.Error(", "b.Errorf(", "b.Fatal(", "b.Fatalf(",
        "b.Fail(", "b.FailNow(",
        "f.Error(", "f.Errorf(", "f.Fatal(", "f.Fatalf(",
    ];
    let std_count = count_patterns(body, &std_assertion_patterns);

    // testify assertions
    let testify_re = Regex::new(r"\b(?:assert|require)\.\w+\(").unwrap();
    let testify_count = count_regex(body, &testify_re);

    // gomega: Expect(...).To(...) / Ω(...)
    let gomega_re = Regex::new(r"(?:Expect|Ω)\s*\(").unwrap();
    let gomega_count = count_regex(body, &gomega_re);

    let total_assertion_count = std_count + testify_count + gomega_count;
    let has_assertion = total_assertion_count > 0;

    // sleep: time.Sleep
    let has_sleep = has_pattern(body, &["time.Sleep("]);

    // 条件分岐
    let has_if = body.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && (t.starts_with("if ") || t.starts_with("if(") || t == "if")
    });
    let has_switch = body.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && (t.starts_with("switch ") || t.starts_with("switch{") || t == "switch")
    });
    let has_select = body.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && (t.starts_with("select ") || t.starts_with("select{") || t == "select")
    });
    let has_for = body.lines().any(|l| {
        let t = l.trim();
        !t.starts_with("//") && (t.starts_with("for ") || t.starts_with("for{") || t == "for")
    });
    let has_conditional = has_if || has_switch || has_select || has_for;
    let has_branching = has_if || has_switch || has_select;

    // print 検出: fmt.Print*, log.Print*, t.Log*, println
    let has_print = has_pattern(body, &[
        "fmt.Print(", "fmt.Println(", "fmt.Printf(",
        "log.Print(", "log.Println(", "log.Printf(",
        "t.Log(", "t.Logf(",
        "println(",
    ]);

    // early return (先頭3行以内の return)
    let has_early_return = body.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
        .take(3)
        .any(|l| {
            let t = l.trim();
            t.starts_with("return")
        });

    // timeout 依存: time.After, time.NewTimer, time.NewTicker, context.WithTimeout, context.WithDeadline
    let has_timeout_dependency = has_pattern(body, &[
        "time.After(", "time.NewTimer(", "time.NewTicker(",
        "time.AfterFunc(", "context.WithTimeout(", "context.WithDeadline(",
        "time.Now()", "time.Since(",
    ]);

    // magic numbers (アサーション行内の大きな数値)
    let magic_numbers = extract_magic_numbers(body);

    // for ループ内のアサーション（テーブル駆動テストの兆候）
    let has_assertion_in_loop = check_assertion_in_loop(body);

    // メッセージなしアサーション（Go の t.Error/t.Fatal は引数なしで呼ぶと空メッセージ）
    let assertions_without_message = count_assertions_without_message(body);

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
        assert_only_count: 0, // Go では区別なし
        assertions_without_message,
        assert_only_without_message: 0,
        magic_numbers,
        has_early_return,
        has_timeout_dependency,
        body_line_count,
    }
}

/// ボディ内に指定パターンがあるか（コメント行は除外）
fn has_pattern(body: &str, patterns: &[&str]) -> bool {
    body.lines().any(|l| {
        let t = l.trim();
        if t.starts_with("//") {
            return false;
        }
        patterns.iter().any(|p| t.contains(p))
    })
}

/// パターンの出現回数をカウント（コメント行は除外）
fn count_patterns(body: &str, patterns: &[&str]) -> usize {
    let mut count = 0;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        for pattern in patterns {
            if t.contains(pattern) {
                count += 1;
                break; // 1行につき1カウント
            }
        }
    }
    count
}

/// 正規表現にマッチする行数をカウント（コメント行は除外）
fn count_regex(body: &str, re: &Regex) -> usize {
    body.lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//") && re.is_match(t)
        })
        .count()
}

/// メッセージなしアサーションのカウント
/// t.Fail() と t.FailNow() は引数なし。t.Error() / t.Fatal() に引数がない場合。
fn count_assertions_without_message(body: &str) -> usize {
    let no_msg_re = Regex::new(r"\b[tbf]\.(Fail|FailNow)\s*\(\s*\)").unwrap();
    body.lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//") && no_msg_re.is_match(t)
        })
        .count()
}

/// アサーション行から大きなマジックナンバーを抽出
fn extract_magic_numbers(body: &str) -> Vec<(i64, usize)> {
    let num_re = Regex::new(r"\b(\d+)\b").unwrap();
    let assertion_re = Regex::new(
        r"(?:assert\.|require\.|t\.Error|t\.Fatal|t\.Fail|Expect|Ω|==\s*\d|!=\s*\d)"
    ).unwrap();
    let mut result = Vec::new();

    for (i, line) in body.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        // アサーション系 or 比較演算を含む行のみ
        if assertion_re.is_match(t) {
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

/// for ループ内にアサーションがあるかの簡易チェック（テーブル駆動テストの兆候）
fn check_assertion_in_loop(body: &str) -> bool {
    let mut in_for = false;
    let mut depth: i32 = 0;

    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }

        if t.starts_with("for ") || t.starts_with("for{") || t == "for" {
            in_for = true;
            depth = 0;
        }

        if in_for {
            depth += t.matches('{').count() as i32;
            depth -= t.matches('}').count() as i32;

            // アサーションがあるか
            if t.contains("t.Error") || t.contains("t.Fatal") || t.contains("t.Fail")
                || t.contains("assert.") || t.contains("require.")
                || t.contains("Expect(") || t.contains("Ω(")
            {
                return true;
            }

            if depth <= 0 {
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
    fn test_basic_test_function() {
        let source = r#"
package foo_test

import "testing"

func TestAdd(t *testing.T) {
    result := Add(2, 3)
    if result != 5 {
        t.Errorf("expected 5, got %d", result)
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "TestAdd");
        assert!(fns[0].has_assertion);
        assert!(!fns[0].is_empty);
        assert!(fns[0].has_branching); // if
    }

    #[test]
    fn test_testify_assertions() {
        let source = r#"
package foo_test

import (
    "testing"
    "github.com/stretchr/testify/assert"
)

func TestWithTestify(t *testing.T) {
    result := Add(2, 3)
    assert.Equal(t, 5, result)
    assert.NoError(t, nil)
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion);
        assert_eq!(fns[0].assertion_count, 2);
    }

    #[test]
    fn test_empty_test() {
        let source = r#"
package foo_test

import "testing"

func TestEmpty(t *testing.T) {
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_empty);
    }

    #[test]
    fn test_missing_assertion() {
        let source = r#"
package foo_test

import "testing"

func TestNoAssert(t *testing.T) {
    result := Add(2, 3)
    _ = result
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(!fns[0].has_assertion);
    }

    #[test]
    fn test_sleepy_test() {
        let source = r#"
package foo_test

import (
    "testing"
    "time"
)

func TestSleepy(t *testing.T) {
    time.Sleep(2 * time.Second)
    if result != expected {
        t.Fatal("mismatch")
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_sleep);
    }

    #[test]
    fn test_skip_detection() {
        let source = r#"
package foo_test

import "testing"

func TestSkipped(t *testing.T) {
    t.Skip("not ready yet")
    if 1 != 1 {
        t.Fatal("impossible")
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_ignored);
    }

    #[test]
    fn test_table_driven_test() {
        let source = r#"
package foo_test

import "testing"

func TestTableDriven(t *testing.T) {
    tests := []struct {
        input    int
        expected int
    }{
        {1, 2},
        {2, 4},
        {3, 6},
    }
    for _, tt := range tests {
        t.Run("", func(t *testing.T) {
            if got := Double(tt.input); got != tt.expected {
                t.Errorf("Double(%d) = %d, want %d", tt.input, got, tt.expected)
            }
        })
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_for_loop);
        assert!(fns[0].has_assertion_in_loop);
    }

    #[test]
    fn test_benchmark_function() {
        let source = r#"
package foo_test

import "testing"

func BenchmarkAdd(b *testing.B) {
    for i := 0; i < b.N; i++ {
        Add(2, 3)
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "BenchmarkAdd");
    }

    #[test]
    fn test_print_detection() {
        let source = r#"
package foo_test

import (
    "fmt"
    "testing"
)

func TestWithPrint(t *testing.T) {
    result := Add(2, 3)
    fmt.Println("debug:", result)
    t.Log("also debug")
    if result != 5 {
        t.Error("wrong")
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_print);
    }

    #[test]
    fn test_conditional_logic() {
        let source = r#"
package foo_test

import "testing"

func TestWithSwitch(t *testing.T) {
    switch runtime.GOOS {
    case "linux":
        t.Error("linux only")
    default:
        t.Skip("unsupported")
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_conditional);
        assert!(fns[0].has_branching);
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"package foo_test

import "testing"

func TestFirst(t *testing.T) {
    t.Error("fail")
}

func TestSecond(t *testing.T) {
    t.Error("fail")
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].line, 5); // 1-indexed
        assert_eq!(fns[1].line, 9);
    }

    #[test]
    fn test_timeout_dependency() {
        let source = r#"
package foo_test

import (
    "context"
    "testing"
    "time"
)

func TestTimeout(t *testing.T) {
    ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
    defer cancel()
    result := DoSomething(ctx)
    if result != expected {
        t.Fatal("timeout or wrong result")
    }
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_timeout_dependency);
    }

    #[test]
    fn test_comment_only_body() {
        let source = r#"
package foo_test

import "testing"

func TestCommentsOnly(t *testing.T) {
    // TODO: implement this test
    // another comment
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_empty);
    }

    #[test]
    fn test_non_test_file_rejected() {
        let parser = GoParser;
        let source = r#"
package foo

func Add(a, b int) int {
    return a + b
}
"#;
        // 非テストファイルはパースしない
        assert!(parser.parse("foo.go", source).is_none());
        // テストファイルはパースする
        assert!(parser.parse("foo_test.go", source).is_some());
    }

    #[test]
    fn test_example_function() {
        let source = r#"
package foo_test

import "fmt"

func ExampleAdd() {
    fmt.Println(Add(2, 3))
    // Output: 5
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "ExampleAdd");
    }

    #[test]
    fn test_fuzz_function() {
        let source = r#"
package foo_test

import "testing"

func FuzzAdd(f *testing.F) {
    f.Add(1, 2)
    f.Fuzz(func(t *testing.T, a, b int) {
        result := Add(a, b)
        if result != a+b {
            t.Errorf("Add(%d, %d) = %d", a, b, result)
        }
    })
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "FuzzAdd");
    }

    #[test]
    fn test_gomega_assertions() {
        let source = r#"
package foo_test

import (
    "testing"
    . "github.com/onsi/gomega"
)

func TestGomega(t *testing.T) {
    RegisterTestingT(t)
    Expect(Add(2, 3)).To(Equal(5))
    Ω(Subtract(5, 3)).Should(Equal(2))
}
"#;
        let fns = extract_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion);
        assert_eq!(fns[0].assertion_count, 2);
    }
}
