use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use tree_sitter::{Node, Parser};

pub struct GoParser;

impl LanguageParser for GoParser {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn language_name(&self) -> &str {
        "go"
    }

    fn parse(&self, path: &str, source: &str) -> Option<TestFile> {
        if !path.ends_with("_test.go") {
            return None;
        }

        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into()).ok()?;
        let tree = parser.parse(source, None)?;
        let root = tree.root_node();

        let test_functions = extract_test_functions(&root, source.as_bytes());

        Some(TestFile {
            path: path.to_string(),
            language: "go".to_string(),
            test_functions,
            source: Some(source.to_string()),
        })
    }
}

/// tree-sitter AST からテスト関数を抽出する
fn extract_test_functions(root: &Node, source: &[u8]) -> Vec<TestFunction> {
    let mut functions = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() != "function_declaration" {
            continue;
        }

        let name = match child.child_by_field_name("name") {
            Some(n) => node_text(&n, source),
            None => continue,
        };

        // Test*, Benchmark*, Fuzz*, Example* のみ対象
        if !name.starts_with("Test") && !name.starts_with("Benchmark")
            && !name.starts_with("Fuzz") && !name.starts_with("Example")
        {
            continue;
        }

        let body_node = match child.child_by_field_name("body") {
            Some(b) => b,
            None => continue,
        };

        let line = child.start_position().row + 1; // 1-indexed
        let test_fn = analyze_function(&name, line, &body_node, source);
        functions.push(test_fn);
    }

    functions
}

/// 関数ボディの AST を解析して TestFunction を生成
fn analyze_function(name: &str, line: usize, body: &Node, source: &[u8]) -> TestFunction {
    let body_source = node_text(body, source);
    // ボディの行数（開き/閉じの波括弧行を除く）
    let body_line_count = body.end_position().row.saturating_sub(body.start_position().row + 1);

    let stmts = find_statement_list(body);

    // 空テスト: statement_list が無い or コメントのみ
    let is_empty = match &stmts {
        None => true,
        Some(sl) => {
            let mut c = sl.walk();
            let result = sl.children(&mut c).all(|n: Node| n.kind() == "comment");
            result
        }
    };

    // AST 全体を走査してパターンを収集
    let mut info = FunctionInfo::default();
    if let Some(ref sl) = stmts {
        walk_statements(sl, source, &mut info, false);
    }

    // early return: 最初の3つの非コメント文に return があるか
    let has_early_return = if let Some(ref sl) = stmts {
        let mut c = sl.walk();
        let result = sl.children(&mut c)
            .filter(|n: &Node| n.kind() != "comment")
            .take(3)
            .any(|n: Node| contains_kind(&n, "return_statement"));
        result
    } else {
        false
    };

    TestFunction {
        name: name.to_string(),
        line,
        body_source,
        is_ignored: info.has_skip,
        has_assertion: info.assertion_count > 0,
        has_sleep: info.has_sleep,
        has_conditional: info.has_if || info.has_switch || info.has_select || info.has_for,
        has_branching: info.has_if || info.has_switch || info.has_select,
        has_for_loop: info.has_for,
        has_assertion_in_loop: info.has_assertion_in_loop,
        has_print: info.has_print,
        is_empty,
        assertion_count: info.assertion_count,
        assert_only_count: 0,
        assertions_without_message: info.assertions_without_message,
        assert_only_without_message: 0,
        magic_numbers: info.magic_numbers,
        has_early_return,
        has_timeout_dependency: info.has_timeout_dependency,
        body_line_count,
    }
}

/// 関数ボディの解析情報
#[derive(Default)]
struct FunctionInfo {
    assertion_count: usize,
    assertions_without_message: usize,
    has_skip: bool,
    has_sleep: bool,
    has_print: bool,
    has_if: bool,
    has_switch: bool,
    has_select: bool,
    has_for: bool,
    has_timeout_dependency: bool,
    has_assertion_in_loop: bool,
    magic_numbers: Vec<(i64, usize)>,
}

/// statement_list を再帰的に走査し、パターンを収集する
fn walk_statements(node: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "if_statement" => {
                info.has_if = true;
                walk_statements(&child, source, info, in_loop);
            }
            "expression_switch_statement" | "type_switch_statement" => {
                info.has_switch = true;
                walk_statements(&child, source, info, in_loop);
            }
            "select_statement" => {
                info.has_select = true;
                walk_statements(&child, source, info, in_loop);
            }
            "for_statement" => {
                info.has_for = true;
                walk_statements(&child, source, info, true);
            }
            "expression_statement" => {
                if let Some(call) = child.child(0) {
                    if call.kind() == "call_expression" {
                        check_call(&call, source, info, in_loop);
                    }
                }
            }
            "short_var_declaration" | "assignment_statement" => {
                // 右辺に call_expression があるかチェック（例: ctx, cancel := context.WithTimeout(...)）
                walk_for_calls(&child, source, info, in_loop);
            }
            "block" | "statement_list" | "expression_case" | "default_case"
            | "communication_case" | "defer_statement" => {
                walk_statements(&child, source, info, in_loop);
            }
            _ => {
                // その他のノード内の call_expression も探す
                walk_for_calls(&child, source, info, in_loop);
            }
        }
    }
}

/// ノード内の全 call_expression を再帰的に探す
fn walk_for_calls(node: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_expression" {
            check_call(&child, source, info, in_loop);
        } else {
            walk_for_calls(&child, source, info, in_loop);
        }
    }
}

/// call_expression を分類する
fn check_call(call: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    if let Some(func_node) = call.child(0) {
        match func_node.kind() {
            "selector_expression" => {
                // メソッドチェーン: Expect(...).To(Equal(5))
                // selector_expression の左辺が call_expression の場合、それも再帰的にチェック
                if let Some(left) = func_node.child(0) {
                    if left.kind() == "call_expression" {
                        check_call(&left, source, info, in_loop);
                    } else {
                        let obj = node_text(&left, source);
                        let method = func_node.child_by_field_name("field")
                            .map(|n| node_text(&n, source))
                            .unwrap_or_default();
                        check_selector_call(&obj, &method, call, source, info, in_loop);
                    }
                }
            }
            "identifier" => {
                let func_name = node_text(&func_node, source);
                // println(...)
                if func_name == "println" {
                    info.has_print = true;
                }
                // gomega: Expect(...) / Ω(...)
                if func_name == "Expect" || func_name == "Ω" {
                    info.assertion_count += 1;
                    if in_loop { info.has_assertion_in_loop = true; }
                }
            }
            _ => {}
        }
    }

    // 引数内の call_expression も再帰的にチェック
    if let Some(args) = call.child_by_field_name("arguments") {
        walk_for_calls(&args, source, info, in_loop);
    }
}

/// selector 呼び出し（obj.method(...)）を分類する
fn check_selector_call(obj: &str, method: &str, call: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    // テスト変数 (t, b, f) のメソッド
    if obj == "t" || obj == "b" || obj == "f" {
        match method {
            // アサーション
            "Error" | "Errorf" | "Fatal" | "Fatalf" => {
                info.assertion_count += 1;
                if in_loop { info.has_assertion_in_loop = true; }
                // 引数なし（t 以外の引数が0個）= メッセージなし
                let arg_count = count_call_arguments(call, source);
                if arg_count == 0 {
                    info.assertions_without_message += 1;
                }
            }
            "Fail" | "FailNow" => {
                info.assertion_count += 1;
                info.assertions_without_message += 1;
                if in_loop { info.has_assertion_in_loop = true; }
            }
            // スキップ
            "Skip" | "SkipNow" | "Skipf" => {
                info.has_skip = true;
            }
            // print
            "Log" | "Logf" => {
                info.has_print = true;
            }
            _ => {}
        }

        // magic number: アサーションの引数に大きな int_literal
        if matches!(method, "Error" | "Errorf" | "Fatal" | "Fatalf" | "Fail" | "FailNow") {
            collect_magic_numbers_from_call(call, source, info);
        }
    }

    // testify: assert.* / require.*
    if obj == "assert" || obj == "require" {
        info.assertion_count += 1;
        if in_loop { info.has_assertion_in_loop = true; }
        // メッセージ判定: 最後の引数が文字列リテラルならメッセージあり
        if !has_message_argument(call, source) {
            info.assertions_without_message += 1;
        }
        collect_magic_numbers_from_call(call, source, info);
    }

    // time パッケージ
    if obj == "time" {
        match method {
            "Sleep" => info.has_sleep = true,
            "After" | "NewTimer" | "NewTicker" | "AfterFunc" | "Now" | "Since" => {
                info.has_timeout_dependency = true;
            }
            _ => {}
        }
    }

    // context パッケージ
    if obj == "context" {
        if method == "WithTimeout" || method == "WithDeadline" {
            info.has_timeout_dependency = true;
        }
    }

    // fmt / log パッケージ
    if obj == "fmt" && (method == "Print" || method == "Println" || method == "Printf") {
        info.has_print = true;
    }
    if obj == "log" && (method == "Print" || method == "Println" || method == "Printf") {
        info.has_print = true;
    }
}

/// call_expression の引数ノードの数を数える（カンマとカッコを除く）
fn count_call_arguments(call: &Node, source: &[u8]) -> usize {
    let _ = source; // 将来の拡張のため保持
    if let Some(args) = call.child_by_field_name("arguments") {
        let mut cursor = args.walk();
        args.children(&mut cursor)
            .filter(|n| n.kind() != "," && n.kind() != "(" && n.kind() != ")")
            .count()
    } else {
        0
    }
}

/// 最後の引数が文字列リテラルならメッセージありと判定する
fn has_message_argument(call: &Node, source: &[u8]) -> bool {
    let _ = source; // 将来の拡張のため保持
    if let Some(args) = call.child_by_field_name("arguments") {
        let mut cursor = args.walk();
        let arg_nodes: Vec<_> = args.children(&mut cursor)
            .filter(|n| n.kind() != "," && n.kind() != "(" && n.kind() != ")")
            .collect();
        // 最後の引数が文字列リテラルなら msg あり
        if let Some(last) = arg_nodes.last() {
            return last.kind() == "interpreted_string_literal"
                || last.kind() == "raw_string_literal";
        }
    }
    false
}

/// call の引数ツリーからマジックナンバーを収集する
fn collect_magic_numbers_from_call(call: &Node, source: &[u8], info: &mut FunctionInfo) {
    collect_int_literals(call, source, &mut info.magic_numbers);
}

/// ノード内の int_literal を再帰的に収集
fn collect_int_literals(node: &Node, source: &[u8], out: &mut Vec<(i64, usize)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "int_literal" {
            let text = node_text(&child, source);
            if let Ok(n) = text.parse::<i64>() {
                if !(-10..=10).contains(&n) {
                    out.push((n, child.start_position().row + 1));
                }
            }
        } else {
            collect_int_literals(&child, source, out);
        }
    }
}

/// statement_list ノードを block 内から取得
fn find_statement_list<'a>(block: &'a Node<'a>) -> Option<Node<'a>> {
    let mut cursor = block.walk();
    for child in block.children(&mut cursor) {
        if child.kind() == "statement_list" {
            return Some(child);
        }
    }
    None
}

/// ノード内に指定 kind のノードが含まれるか
fn contains_kind(node: &Node, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_kind(&child, kind) {
            return true;
        }
    }
    false
}

/// ノードのテキストを取得
fn node_text(node: &Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test_functions(source: &str) -> Vec<TestFunction> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        extract_test_functions(&tree.root_node(), source.as_bytes())
    }

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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
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
        assert!(parser.parse("foo.go", source).is_none());
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
        let fns = parse_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "ExampleAdd");
    }

    #[test]
    fn test_assertion_roulette_testify() {
        // メッセージなしの assert が複数
        let source = r#"
package foo_test

import (
    "testing"
    "github.com/stretchr/testify/assert"
)

func TestRouletteTestify(t *testing.T) {
    assert.Equal(t, 1, Add(0, 1))
    assert.Equal(t, 2, Add(1, 1))
    assert.Equal(t, 3, Add(1, 2))
}
"#;
        let fns = parse_test_functions(source);
        assert_eq!(fns[0].assertions_without_message, 3);
    }

    #[test]
    fn test_assertion_with_message_testify() {
        // メッセージありの assert
        let source = r#"
package foo_test

import (
    "testing"
    "github.com/stretchr/testify/assert"
)

func TestWithMessage(t *testing.T) {
    assert.Equal(t, 1, Add(0, 1), "zero plus one should be one")
    assert.Equal(t, 2, Add(1, 1))
}
"#;
        let fns = parse_test_functions(source);
        assert_eq!(fns[0].assertions_without_message, 1); // 2つ目だけメッセージなし
    }

    #[test]
    fn test_t_error_without_message() {
        // t.Error() は引数0個でメッセージなし
        let source = r#"
package foo_test

import "testing"

func TestErrorNoMsg(t *testing.T) {
    t.Error()
}
"#;
        let fns = parse_test_functions(source);
        assert_eq!(fns[0].assertions_without_message, 1);
    }

    #[test]
    fn test_t_error_with_message() {
        // t.Error("msg") は引数1つでメッセージあり
        let source = r#"
package foo_test

import "testing"

func TestErrorWithMsg(t *testing.T) {
    t.Error("something went wrong")
}
"#;
        let fns = parse_test_functions(source);
        assert_eq!(fns[0].assertions_without_message, 0);
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
        let fns = parse_test_functions(source);
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
        let fns = parse_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion);
        assert_eq!(fns[0].assertion_count, 2);
    }

    #[test]
    fn test_string_not_confused_with_code() {
        // regex では文字列内の "t.Error" を誤検出していた
        let source = r#"
package foo_test

import "testing"

func TestStringContent(t *testing.T) {
    msg := "calling t.Error would be bad"
    _ = msg
}
"#;
        let fns = parse_test_functions(source);
        assert_eq!(fns.len(), 1);
        // 文字列内の "t.Error" をアサーションと誤検出しないこと
        assert!(!fns[0].has_assertion);
    }

    #[test]
    fn test_comment_not_confused_with_code() {
        let source = r#"
package foo_test

import "testing"

func TestCommentedCode(t *testing.T) {
    // t.Error("this is commented out")
    // time.Sleep(1)
    _ = 1
}
"#;
        let fns = parse_test_functions(source);
        assert_eq!(fns.len(), 1);
        assert!(!fns[0].has_assertion);
        assert!(!fns[0].has_sleep);
    }
}
