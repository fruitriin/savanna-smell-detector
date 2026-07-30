use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use tree_sitter::{Node, Parser};

pub struct PythonParser;

impl LanguageParser for PythonParser {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn language_name(&self) -> &str {
        "python"
    }

    fn parse(&self, path: &str, source: &str) -> Option<TestFile> {
        // テストファイルの命名規則: ファイル名に test を含む（test_*.py / *_test.py / tests.py 等）
        let basename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if !basename.to_lowercase().contains("test") {
            return None;
        }

        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_python::LANGUAGE.into()).ok()?;
        let tree = parser.parse(source, None)?;
        let root = tree.root_node();

        let mut test_functions = Vec::new();
        collect_test_functions(&root, source.as_bytes(), &mut test_functions, false);

        Some(TestFile {
            path: path.to_string(),
            language: "python".to_string(),
            test_functions,
            source: Some(source.to_string()),
            flavor: None,
        })
    }
}

/// module / class ボディを再帰的に走査してテスト関数を収集する
/// `class_ignored`: クラスにスキップデコレーターが付いている場合 true
fn collect_test_functions(node: &Node, source: &[u8], out: &mut Vec<TestFunction>, class_ignored: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                maybe_add_function(&child, &[], source, out, class_ignored);
            }
            "decorated_definition" => {
                let mut dec_cursor = child.walk();
                let decorators: Vec<Node> = child.children(&mut dec_cursor)
                    .filter(|n| n.kind() == "decorator")
                    .collect();
                if let Some(def) = child.child_by_field_name("definition") {
                    match def.kind() {
                        "function_definition" => {
                            maybe_add_function(&def, &decorators, source, out, class_ignored);
                        }
                        "class_definition" => {
                            let ignored = class_ignored
                                || decorators.iter().any(|d| is_skip_decorator(d, source));
                            if let Some(body) = def.child_by_field_name("body") {
                                collect_test_functions(&body, source, out, ignored);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "class_definition" => {
                if let Some(body) = child.child_by_field_name("body") {
                    collect_test_functions(&body, source, out, class_ignored);
                }
            }
            _ => {}
        }
    }
}

/// test で始まる名前の関数なら TestFunction として追加する
fn maybe_add_function(
    def: &Node,
    decorators: &[Node],
    source: &[u8],
    out: &mut Vec<TestFunction>,
    class_ignored: bool,
) {
    let name = match def.child_by_field_name("name") {
        Some(n) => node_text(&n, source),
        None => return,
    };

    // pytest / unittest の規約: test で始まる関数・メソッドのみ対象
    if !name.starts_with("test") {
        return;
    }

    let body = match def.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };

    let ignored_by_decorator = class_ignored
        || decorators.iter().any(|d| is_skip_decorator(d, source));

    let line = def.start_position().row + 1; // 1-indexed
    let test_fn = analyze_function(&name, line, &body, source, ignored_by_decorator);
    out.push(test_fn);
}

/// スキップ系デコレーターかどうか判定する
/// @unittest.skip / @pytest.mark.skip(if) / @pytest.mark.xfail / @skip / @skipIf / @skipUnless / @expectedFailure
fn is_skip_decorator(decorator: &Node, source: &[u8]) -> bool {
    let text = node_text(decorator, source);
    let text = text.trim_start_matches('@').trim();
    text.starts_with("unittest.skip")
        || text.starts_with("pytest.mark.skip")
        || text.starts_with("pytest.mark.xfail")
        || text.starts_with("skip")
        || text.starts_with("skipIf")
        || text.starts_with("skipUnless")
        || text.starts_with("expectedFailure")
        || text.starts_with("unittest.expectedFailure")
}

/// 関数ボディ（block）を解析して TestFunction を生成
fn analyze_function(
    name: &str,
    line: usize,
    body: &Node,
    source: &[u8],
    ignored_by_decorator: bool,
) -> TestFunction {
    let body_source = node_text(body, source);

    // 非コメントの文のリスト
    let mut cursor = body.walk();
    let statements: Vec<Node> = body.children(&mut cursor)
        .filter(|n| n.kind() != "comment")
        .collect();

    // docstring: 先頭の文が文字列のみの expression_statement
    let docstring = statements.first().filter(|s| is_docstring(s));
    let docstring_lines = docstring
        .map(|d| d.end_position().row - d.start_position().row + 1)
        .unwrap_or(0);

    // ボディ行数（docstring を除く）
    let raw_line_count = body.end_position().row - body.start_position().row + 1;
    let body_line_count = raw_line_count.saturating_sub(docstring_lines);

    // 空テスト: docstring を除いた文が pass / ... のみ（または無い）
    let is_empty = statements.iter()
        .skip(if docstring.is_some() { 1 } else { 0 })
        .all(|s| matches!(s.kind(), "pass_statement")
            || (s.kind() == "expression_statement" && s.child(0).map_or(false, |c| c.kind() == "ellipsis")));

    let mut info = FunctionInfo::default();
    walk_statements(body, source, &mut info, false);

    // early return: 最初の3つの非コメント文に return があるか
    let has_early_return = statements.iter()
        .take(3)
        .any(|n| contains_kind(n, "return_statement"));

    TestFunction {
        name: name.to_string(),
        line,
        body_source,
        is_ignored: ignored_by_decorator || info.has_skip,
        has_assertion: info.assertion_count > 0,
        has_sleep: info.has_sleep,
        has_conditional: info.has_if || info.has_match || info.has_for || info.has_while,
        has_branching: info.has_if || info.has_match,
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

/// docstring（文字列のみの expression_statement）かどうか
fn is_docstring(stmt: &Node) -> bool {
    stmt.kind() == "expression_statement"
        && stmt.named_child_count() == 1
        && stmt.child(0).map_or(false, |c| c.kind() == "string")
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
    has_match: bool,
    has_for: bool,
    has_while: bool,
    has_timeout_dependency: bool,
    has_assertion_in_loop: bool,
    magic_numbers: Vec<(i64, usize)>,
}

/// 文を再帰的に走査し、パターンを収集する
fn walk_statements(node: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "if_statement" => {
                info.has_if = true;
                walk_statements(&child, source, info, in_loop);
            }
            "match_statement" => {
                info.has_match = true;
                walk_statements(&child, source, info, in_loop);
            }
            "for_statement" => {
                info.has_for = true;
                walk_statements(&child, source, info, true);
            }
            "while_statement" => {
                info.has_while = true;
                walk_statements(&child, source, info, true);
            }
            "assert_statement" => {
                info.assertion_count += 1;
                if in_loop { info.has_assertion_in_loop = true; }
                // assert expr, "message" — 名前付き子ノードが2つ以上ならメッセージあり
                if child.named_child_count() < 2 {
                    info.assertions_without_message += 1;
                }
                collect_magic_numbers(&child, source, &mut info.magic_numbers);
            }
            "raise_statement" => {
                let text = node_text(&child, source);
                if text.contains("SkipTest") {
                    info.has_skip = true;
                } else if text.contains("AssertionError") {
                    info.assertion_count += 1;
                    if in_loop { info.has_assertion_in_loop = true; }
                }
            }
            // ブロックと複合文はさらに文として走査する（内側の if/assert 等を拾う）
            "block" | "with_statement" | "try_statement"
            | "elif_clause" | "else_clause" | "except_clause"
            | "finally_clause" | "case_clause" => {
                walk_statements(&child, source, info, in_loop);
            }
            _ => {
                walk_for_calls(&child, source, info, in_loop);
            }
        }
    }
}

/// ノード内の全 call を再帰的に探す（ネストした文は除く）
fn walk_for_calls(node: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call" {
            check_call(&child, source, info, in_loop);
            // 引数内のネストした call も探す
            walk_for_calls(&child, source, info, in_loop);
        } else if child.kind() != "block" {
            walk_for_calls(&child, source, info, in_loop);
        }
    }
}

/// call ノードを分類する
fn check_call(call: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let func_node = match call.child_by_field_name("function") {
        Some(f) => f,
        None => return,
    };

    match func_node.kind() {
        "attribute" => {
            let obj = func_node.child_by_field_name("object")
                .map(|n| node_text(&n, source))
                .unwrap_or_default();
            let method = func_node.child_by_field_name("attribute")
                .map(|n| node_text(&n, source))
                .unwrap_or_default();
            check_attribute_call(&obj, &method, call, source, info, in_loop);
        }
        "identifier" => {
            let func_name = node_text(&func_node, source);
            let lower = func_name.to_lowercase();
            match func_name.as_str() {
                "print" | "pprint" => info.has_print = true,
                "sleep" => info.has_sleep = true,
                // from pytest import raises / from unittest.mock import ...
                "raises" => {
                    info.assertion_count += 1;
                    if in_loop { info.has_assertion_in_loop = true; }
                }
                _ => {
                    // ヒューリスティック: assert を含む関数名（numpy の assert_array_equal 等）
                    // expect/check/verify で始まるヘルパー関数
                    if lower.contains("assert")
                        || lower.starts_with("expect")
                        || lower.starts_with("check")
                        || lower.starts_with("verify")
                    {
                        info.assertion_count += 1;
                        if in_loop { info.has_assertion_in_loop = true; }
                        collect_magic_numbers(call, source, &mut info.magic_numbers);
                    }
                }
            }
        }
        _ => {}
    }
}

/// attribute 呼び出し（obj.method(...)）を分類する
fn check_attribute_call(
    obj: &str,
    method: &str,
    call: &Node,
    source: &[u8],
    info: &mut FunctionInfo,
    in_loop: bool,
) {
    // unittest: self.assertXxx / self.fail
    if obj == "self" {
        if method.starts_with("assert") {
            info.assertion_count += 1;
            if in_loop { info.has_assertion_in_loop = true; }
            if !has_message_argument(call, source) {
                info.assertions_without_message += 1;
            }
            collect_magic_numbers(call, source, &mut info.magic_numbers);
        } else if method == "fail" {
            info.assertion_count += 1;
            if in_loop { info.has_assertion_in_loop = true; }
            if count_call_arguments(call) == 0 {
                info.assertions_without_message += 1;
            }
        } else if method == "skipTest" {
            info.has_skip = true;
        }
        return;
    }

    // pytest.*
    if obj == "pytest" {
        match method {
            "skip" | "xfail" | "importorskip" => info.has_skip = true,
            "fail" => {
                info.assertion_count += 1;
                if in_loop { info.has_assertion_in_loop = true; }
            }
            "raises" | "warns" | "deprecated_call" => {
                info.assertion_count += 1;
                if in_loop { info.has_assertion_in_loop = true; }
            }
            _ => {}
        }
        return;
    }

    // time / asyncio
    if obj == "time" {
        match method {
            "sleep" => info.has_sleep = true,
            "time" | "monotonic" | "perf_counter" | "time_ns" | "monotonic_ns" => {
                info.has_timeout_dependency = true;
            }
            _ => {}
        }
        return;
    }
    if obj == "asyncio" && method == "sleep" {
        info.has_sleep = true;
        return;
    }

    // datetime.now() / datetime.utcnow() / datetime.datetime.now()
    if (obj == "datetime" || obj.ends_with(".datetime") || obj == "date")
        && matches!(method, "now" | "utcnow" | "today")
    {
        info.has_timeout_dependency = true;
        return;
    }

    // unittest.mock: m.assert_called_once_with(...) 等
    if method.starts_with("assert_") {
        info.assertion_count += 1;
        if in_loop { info.has_assertion_in_loop = true; }
        return;
    }

    // unittest.TestCase 継承以外の変数経由: tc.assertEqual(...) 等
    if method.starts_with("assert") {
        info.assertion_count += 1;
        if in_loop { info.has_assertion_in_loop = true; }
    }
}

/// call の引数の数を数える（キーワード引数含む）
fn count_call_arguments(call: &Node) -> usize {
    call.child_by_field_name("arguments")
        .map(|args| args.named_child_count())
        .unwrap_or(0)
}

/// メッセージ引数があるか: msg= キーワード引数、または最後の位置引数が文字列リテラル
fn has_message_argument(call: &Node, source: &[u8]) -> bool {
    let args = match call.child_by_field_name("arguments") {
        Some(a) => a,
        None => return false,
    };
    let mut cursor = args.walk();
    let arg_nodes: Vec<Node> = args.children(&mut cursor)
        .filter(|n| n.is_named())
        .collect();

    // msg= キーワード引数
    for arg in &arg_nodes {
        if arg.kind() == "keyword_argument" {
            if let Some(name) = arg.child_by_field_name("name") {
                if node_text(&name, source) == "msg" {
                    return true;
                }
            }
        }
    }

    // 引数が3つ以上で最後が文字列リテラル（assertEqual(a, b, "msg") パターン）
    if arg_nodes.len() >= 3 {
        if let Some(last) = arg_nodes.last() {
            return last.kind() == "string";
        }
    }
    false
}

/// ノード内の integer リテラルからマジックナンバーを収集する
fn collect_magic_numbers(node: &Node, source: &[u8], out: &mut Vec<(i64, usize)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "integer" {
            let text = node_text(&child, source);
            if let Ok(n) = text.parse::<i64>() {
                if !(-10..=10).contains(&n) {
                    out.push((n, child.start_position().row + 1));
                }
            }
        } else if child.kind() != "string" {
            collect_magic_numbers(&child, source, out);
        }
    }
}

/// ノード内に指定 kind のノードがあるか（再帰）
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

fn node_text(node: &Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<TestFunction> {
        PythonParser
            .parse("test_sample.py", source)
            .map(|tf| tf.test_functions)
            .unwrap_or_default()
    }

    #[test]
    fn test_pytest_function_extraction() {
        let source = r#"
def test_addition():
    result = 2 + 2
    assert result == 4
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "test_addition");
        assert!(fns[0].has_assertion);
        assert!(!fns[0].is_empty);
    }

    #[test]
    fn test_non_test_file_returns_none() {
        let source = "def test_foo():\n    assert True\n";
        assert!(PythonParser.parse("utils.py", source).is_none());
        assert!(PythonParser.parse("test_utils.py", source).is_some());
        assert!(PythonParser.parse("utils_test.py", source).is_some());
    }

    #[test]
    fn test_unittest_class_methods() {
        let source = r#"
import unittest

class TestMath(unittest.TestCase):
    def test_addition(self):
        self.assertEqual(2 + 2, 4)

    def test_with_message(self):
        self.assertEqual(2 + 2, 4, "addition should work")

    def helper_method(self):
        pass
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "test_addition");
        assert!(fns[0].has_assertion);
        assert_eq!(fns[0].assertions_without_message, 1);
        assert_eq!(fns[1].assertions_without_message, 0);
    }

    #[test]
    fn test_async_function() {
        let source = r#"
import asyncio

async def test_async_thing():
    await asyncio.sleep(1)
    assert True
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "test_async_thing");
        assert!(fns[0].has_sleep);
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_empty_test_detection() {
        let source = r#"
def test_empty():
    pass

def test_ellipsis():
    ...

def test_docstring_only():
    """This test does nothing."""
    pass
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 3);
        assert!(fns.iter().all(|f| f.is_empty));
    }

    #[test]
    fn test_missing_assertion() {
        let source = r#"
def test_no_assert():
    result = compute()
    print(result)
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert!(!fns[0].has_assertion);
        assert!(fns[0].has_print);
    }

    #[test]
    fn test_sleepy_test() {
        let source = r#"
import time

def test_sleepy():
    time.sleep(2)
    assert server.is_up()
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_sleep);
    }

    #[test]
    fn test_bare_sleep_import() {
        let source = r#"
from time import sleep

def test_sleepy():
    sleep(1)
    assert True
"#;
        let fns = parse(source);
        assert!(fns[0].has_sleep);
    }

    #[test]
    fn test_skip_decorators() {
        let source = r#"
import unittest
import pytest

@unittest.skip("not ready")
def test_skipped_unittest():
    assert True

@pytest.mark.skip(reason="wip")
def test_skipped_pytest():
    assert True

@pytest.mark.skipif(True, reason="conditional")
def test_skipif():
    assert True

def test_not_skipped():
    assert True
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 4);
        assert!(fns[0].is_ignored);
        assert!(fns[1].is_ignored);
        assert!(fns[2].is_ignored);
        assert!(!fns[3].is_ignored);
    }

    #[test]
    fn test_skip_in_body() {
        let source = r#"
import pytest

def test_skip_call():
    pytest.skip("runtime skip")
    assert True

class TestFoo:
    def test_skip_method(self):
        self.skipTest("skip me")
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
        assert!(fns[0].is_ignored);
        assert!(fns[1].is_ignored);
    }

    #[test]
    fn test_class_level_skip() {
        let source = r#"
import unittest

@unittest.skip("whole class")
class TestSkipped(unittest.TestCase):
    def test_inside(self):
        self.assertTrue(True)
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_ignored);
    }

    #[test]
    fn test_conditional_logic() {
        let source = r#"
def test_with_if():
    if condition:
        assert foo == 1
    else:
        assert foo == 2

def test_with_for():
    for i in range(3):
        assert i < 3

def test_with_while():
    while not done():
        pass
    assert done()
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 3);
        assert!(fns[0].has_conditional);
        assert!(fns[0].has_branching);
        assert!(fns[1].has_conditional);
        assert!(!fns[1].has_branching);
        assert!(fns[1].has_for_loop);
        assert!(fns[1].has_assertion_in_loop);
        assert!(fns[2].has_conditional);
    }

    #[test]
    fn test_pytest_raises() {
        let source = r#"
import pytest

def test_raises():
    with pytest.raises(ValueError):
        int("not a number")
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_mock_assertions() {
        let source = r#"
def test_mock_called():
    do_something(mock)
    mock.assert_called_once_with(42)
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_assertion_roulette() {
        let source = r#"
def test_many_bare_asserts():
    assert a == 1
    assert b == 2

def test_asserts_with_messages():
    assert a == 1, "a should be 1"
    assert b == 2, "b should be 2"
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].assertions_without_message, 2);
        assert_eq!(fns[1].assertions_without_message, 0);
    }

    #[test]
    fn test_magic_numbers() {
        let source = r#"
def test_magic():
    assert calculate() == 12345
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].magic_numbers.len(), 1);
        assert_eq!(fns[0].magic_numbers[0].0, 12345);
    }

    #[test]
    fn test_magic_number_in_string_not_counted() {
        let source = r#"
def test_string_number():
    assert name == "user12345"
"#;
        let fns = parse(source);
        assert_eq!(fns[0].magic_numbers.len(), 0);
    }

    #[test]
    fn test_timeout_dependency() {
        let source = r#"
import time

def test_timing():
    start = time.time()
    run_task()
    assert time.time() - start < 5
"#;
        let fns = parse(source);
        assert!(fns[0].has_timeout_dependency);
    }

    #[test]
    fn test_early_return() {
        let source = r#"
def test_early_return():
    if not available():
        return
    assert compute() == 1
"#;
        let fns = parse(source);
        assert!(fns[0].has_early_return);
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"# comment line

def test_first():
    assert True

def test_second():
    assert True
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].line, 3);
        assert_eq!(fns[1].line, 6);
    }

    #[test]
    fn test_decorated_line_number_points_to_def() {
        let source = r#"
import pytest

@pytest.mark.parametrize("x", [1, 2])
def test_parametrized(x):
    assert x > 0
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].line, 5); // def の行（デコレーターではなく）
    }

    #[test]
    fn test_docstring_excluded_from_line_count() {
        let source = r#"
def test_with_docstring():
    """This is a
    multi-line
    docstring."""
    assert True
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].body_line_count, 1); // assert の1行のみ
    }

    #[test]
    fn test_numpy_style_assertion_helper() {
        let source = r#"
from numpy.testing import assert_array_equal

def test_array():
    assert_array_equal(result, expected)
"#;
        let fns = parse(source);
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_raise_assertion_error() {
        let source = r#"
def test_manual_assert():
    if result != expected:
        raise AssertionError("mismatch")
"#;
        let fns = parse(source);
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_nested_class() {
        let source = r#"
class TestOuter:
    class TestInner:
        def test_deep(self):
            assert True

    def test_outer(self):
        assert True
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
    }
}
