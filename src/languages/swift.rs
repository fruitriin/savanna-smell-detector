use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use tree_sitter::{Node, Parser};

pub struct SwiftParser;

impl LanguageParser for SwiftParser {
    fn extensions(&self) -> &[&str] {
        &["swift"]
    }

    fn language_name(&self) -> &str {
        "swift"
    }

    fn parse(&self, path: &str, source: &str) -> Option<TestFile> {
        if !is_test_path(path) {
            return None;
        }

        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_swift::LANGUAGE.into()).ok()?;
        let tree = parser.parse(source, None)?;
        let root = tree.root_node();

        // XCTest の `func testXxx()` 規約は、XCTest を使っているファイルでのみ有効にする。
        // テストファイルに同居するヘルパー型（MockURLProtocol 等）を巻き込まないため。
        let ctx = FileContext {
            has_xctest: source.contains("XCTest"),
        };

        let mut test_functions = Vec::new();
        collect_test_functions(&root, source.as_bytes(), &ctx, &mut test_functions, false, false);

        // XCUITest は「待つ」ことが本質的に必要なので、指摘の文面を切り替える
        let flavor = if source.contains("XCUIApplication") || source.contains("XCUIElement") {
            Some("xcuitest".to_string())
        } else {
            None
        };

        Some(TestFile {
            path: path.to_string(),
            language: "swift".to_string(),
            test_functions,
            source: Some(source.to_string()),
            flavor,
        })
    }
}

struct FileContext {
    has_xctest: bool,
}

/// テストファイルかどうか: ファイル名に test を含むか、Tests/ 配下にある
fn is_test_path(path: &str) -> bool {
    let p = std::path::Path::new(path);
    let basename = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if basename.contains("test") {
        return true;
    }
    p.components().any(|c| {
        let s = c.as_os_str().to_string_lossy().to_lowercase();
        s == "tests" || s == "test" || s.ends_with("tests")
    })
}

/// source_file / class_body を再帰的に走査してテスト関数を集める
/// `type_ignored`: 囲みの型に `@Suite(.disabled)` が付いている場合 true
/// `in_type`: class/struct/enum/extension のボディの中にいる場合 true
fn collect_test_functions(
    node: &Node,
    source: &[u8],
    ctx: &FileContext,
    out: &mut Vec<TestFunction>,
    type_ignored: bool,
    in_type: bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                maybe_add_function(&child, source, ctx, out, type_ignored, in_type);
            }
            // class / struct / enum / extension はすべて class_declaration
            "class_declaration" | "protocol_declaration" => {
                let ignored = type_ignored || has_disabled_attribute(&child, source);
                if let Some(body) = find_child(&child, "class_body") {
                    collect_test_functions(&body, source, ctx, out, ignored, true);
                }
            }
            _ => {}
        }
    }
}

/// テスト関数なら TestFunction として追加する
///
/// 判定は2系統:
///   - Swift Testing: `@Test` 属性が付いている（置き場所は問わない）
///   - XCTest: 型のボディ内にある、引数なしの `test` 始まりの func
fn maybe_add_function(
    decl: &Node,
    source: &[u8],
    ctx: &FileContext,
    out: &mut Vec<TestFunction>,
    type_ignored: bool,
    in_type: bool,
) {
    let name = match find_child(decl, "simple_identifier") {
        Some(n) => node_text(&n, source),
        None => return,
    };

    let attrs = attribute_texts(decl, source);
    let is_swift_testing = attrs.iter().any(|a| attr_name(a) == "Test");
    let has_parameters = find_child(decl, "parameter").is_some();
    let is_xctest = ctx.has_xctest && in_type && name.starts_with("test") && !has_parameters;

    if !is_swift_testing && !is_xctest {
        return;
    }

    let body = match find_child(decl, "function_body") {
        Some(b) => b,
        None => return,
    };

    // `.disabled` は @Test / @Suite の trait。@Test(.disabled("wip")) など
    let ignored_by_attribute = type_ignored || attrs.iter().any(|a| a.contains(".disabled"));

    // function_declaration の開始位置は属性の行を指すので、func キーワードの行に補正する
    let line = find_child(decl, "func")
        .map(|n| n.start_position().row + 1)
        .unwrap_or_else(|| decl.start_position().row + 1);

    out.push(analyze_function(&name, line, &body, source, ignored_by_attribute));
}

/// modifiers 配下の attribute のテキスト一覧（"@Test(...)" 等）
fn attribute_texts(decl: &Node, source: &[u8]) -> Vec<String> {
    let modifiers = match find_child(decl, "modifiers") {
        Some(m) => m,
        None => return Vec::new(),
    };
    let mut cursor = modifiers.walk();
    modifiers
        .children(&mut cursor)
        .filter(|n| n.kind() == "attribute")
        .map(|n| node_text(&n, source))
        .collect()
}

/// "@Test(...)" → "Test"
fn attr_name(attr: &str) -> &str {
    attr.trim_start_matches('@')
        .split(['(', ' ', '\n'])
        .next()
        .unwrap_or("")
}

/// 型宣言に `@Suite(.disabled)` 等が付いているか
fn has_disabled_attribute(decl: &Node, source: &[u8]) -> bool {
    attribute_texts(decl, source)
        .iter()
        .any(|a| a.contains(".disabled"))
}

/// 関数ボディ（function_body）を解析して TestFunction を生成
fn analyze_function(
    name: &str,
    line: usize,
    body: &Node,
    source: &[u8],
    ignored_by_attribute: bool,
) -> TestFunction {
    let body_source = node_text(body, source);
    let body_line_count = body.end_position().row - body.start_position().row + 1;

    // function_body の中身は statements ノード1つ（空なら存在しない）
    let statements_node = find_child(body, "statements");

    let statements: Vec<Node> = match statements_node {
        Some(s) => {
            let mut cursor = s.walk();
            s.children(&mut cursor)
                .filter(|n| n.is_named() && n.kind() != "comment" && n.kind() != "multiline_comment")
                .collect()
        }
        None => Vec::new(),
    };

    let is_empty = statements.is_empty();

    let mut info = FunctionInfo::default();
    if let Some(s) = statements_node {
        walk_statements(&s, source, &mut info, false);
    }

    // 条件付き early return（黙ってテストを終わらせる脱出）があるか
    let has_early_return = statements_node.map_or(false, |s| has_silent_skip(&s, source));

    // Date() 単体はフィクスチャの埋め草でしかないので、時刻演算と組み合わさったときだけ
    // 時間依存とみなす（明示的な待ち API は単体で時間依存）
    let has_timeout_dependency =
        info.has_explicit_wait || (info.has_clock_read && info.has_time_arithmetic);

    TestFunction {
        name: name.to_string(),
        line,
        body_source,
        is_ignored: ignored_by_attribute || info.has_skip,
        has_assertion: info.assertion_count > 0,
        has_sleep: info.has_sleep,
        has_conditional: info.has_if || info.has_switch || info.has_for || info.has_while,
        has_branching: info.has_if || info.has_switch,
        has_for_loop: info.has_for,
        has_assertion_in_loop: info.has_assertion_in_loop,
        has_print: info.has_print,
        is_empty,
        assertion_count: info.assertion_count,
        assert_only_count: info.assert_only_count,
        assertions_without_message: info.assertions_without_message,
        assert_only_without_message: info.assert_only_without_message,
        magic_numbers: info.magic_numbers,
        has_early_return,
        has_timeout_dependency,
        body_line_count,
    }
}

/// `guard ... else { return }` / `if ... { return }` のような条件付き早期 return があるか
///
/// 他言語の実装と違い「先頭の3文」に限定しない。Swift の `guard ... else { return }` は
/// 位置に関係なく必ずそこで脱出する構文で、末尾近くにあっても黙って通ることに変わりないため。
/// `else { throw XCTSkip(...) }` は正しいスキップなので対象外（IgnoredTest として拾う）。
fn has_silent_skip(node: &Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // クロージャ・ネストした関数の return は「テストの脱出」ではない
            "lambda_literal" | "function_declaration" => continue,
            "guard_statement" | "if_statement" => {
                // 失敗を報告してから return するのは「黙って」ではない
                if contains_return(&child) && !reports_failure(&child, source) {
                    return true;
                }
                if has_silent_skip(&child, source) {
                    return true;
                }
            }
            _ => {
                if has_silent_skip(&child, source) {
                    return true;
                }
            }
        }
    }
    false
}

/// 分岐の中でテストの失敗を報告しているか（XCTFail / XCTAssert* / Issue.record / #expect）
fn reports_failure(node: &Node, source: &[u8]) -> bool {
    match node.kind() {
        "macro_invocation" => {
            let name = find_child(node, "simple_identifier")
                .map(|n| node_text(&n, source))
                .unwrap_or_default();
            if name == "expect" || name == "require" {
                return true;
            }
        }
        "call_expression" => {
            if let Some(callee) = node.child(0) {
                let text = node_text(&callee, source);
                let method = text.rsplit('.').next().unwrap_or(&text).trim().to_string();
                if method.starts_with("XCTAssert")
                    || method == "XCTFail"
                    || method == "XCTUnwrap"
                    || (method == "record" && text.starts_with("Issue."))
                {
                    return true;
                }
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .filter(|c| !matches!(c.kind(), "lambda_literal" | "function_declaration"))
        .any(|c| reports_failure(&c, source));
    found
}

/// ノード内に `return` の control_transfer_statement があるか（クロージャの中は見ない）
fn contains_return(node: &Node) -> bool {
    if node.kind() == "control_transfer_statement" {
        let mut cursor = node.walk();
        let is_return = node.children(&mut cursor).any(|c| c.kind() == "return");
        return is_return;
    }
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .filter(|c| !matches!(c.kind(), "lambda_literal" | "function_declaration"))
        .any(|c| contains_return(&c));
    found
}

/// 関数ボディの解析情報
#[derive(Default)]
struct FunctionInfo {
    assertion_count: usize,
    /// 真偽値だけを見るアサーション（XCTAssertTrue 等 = Rust の assert! 相当）
    assert_only_count: usize,
    /// 値を自動表示するアサーションのうちメッセージなし（#expect / XCTAssertEqual 等）
    assertions_without_message: usize,
    /// 真偽値だけのアサーションのうちメッセージなし
    assert_only_without_message: usize,
    has_skip: bool,
    has_sleep: bool,
    has_print: bool,
    has_if: bool,
    has_switch: bool,
    has_for: bool,
    has_while: bool,
    /// waitForExistence(timeout:) / wait(for:timeout:) など明示的な待ち
    has_explicit_wait: bool,
    /// Date() / Date.now / DispatchTime.now() など現在時刻の読み取り
    has_clock_read: bool,
    /// addingTimeInterval / timeIntervalSince など時刻の演算
    has_time_arithmetic: bool,
    has_assertion_in_loop: bool,
    magic_numbers: Vec<(i64, usize)>,
}

/// 文を再帰的に走査してパターンを収集する
fn walk_statements(node: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "if_statement" => {
                info.has_if = true;
                walk_statements(&child, source, info, in_loop);
            }
            "switch_statement" => {
                info.has_switch = true;
                walk_statements(&child, source, info, in_loop);
            }
            "for_statement" => {
                info.has_for = true;
                walk_statements(&child, source, info, true);
            }
            "while_statement" | "repeat_while_statement" => {
                info.has_while = true;
                walk_statements(&child, source, info, true);
            }
            // guard は Swift では分岐というより前提条件の表明なので条件分岐には数えない。
            // 黙って return するものは Silent Skip 側で拾う。
            "guard_statement" => {
                walk_statements(&child, source, info, in_loop);
            }
            "macro_invocation" => {
                check_macro(&child, source, info, in_loop);
                walk_statements(&child, source, info, in_loop);
            }
            "call_expression" => {
                check_call(&child, source, info, in_loop);
                walk_statements(&child, source, info, in_loop);
            }
            _ => {
                walk_statements(&child, source, info, in_loop);
            }
        }
    }
}

/// `#expect(...)` / `#require(...)` を分類する
fn check_macro(node: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let name = match find_child(node, "simple_identifier") {
        Some(n) => node_text(&n, source),
        None => return,
    };
    if name != "expect" && name != "require" {
        return;
    }

    info.assertion_count += 1;
    if in_loop {
        info.has_assertion_in_loop = true;
    }
    // Swift Testing は式を分解して値を表示するので、メッセージなしでも致命的ではない
    // （= Rust の assert_eq! 相当の扱い）。第2引数の Comment があればメッセージあり。
    if argument_count(node) < 2 {
        info.assertions_without_message += 1;
    }
    collect_magic_numbers(node, source, &mut info.magic_numbers);
}

/// 関数・メソッド呼び出しを分類する
fn check_call(call: &Node, source: &[u8], info: &mut FunctionInfo, in_loop: bool) {
    let callee = match call.child(0) {
        Some(c) => node_text(&c, source),
        None => return,
    };
    // "app.buttons[\"x\"].waitForExistence" → "waitForExistence"
    let method = callee.rsplit('.').next().unwrap_or(&callee).trim();
    let args = argument_count(call);

    // --- アサーション ---
    if let Some(kind) = xctest_assertion_kind(method) {
        info.assertion_count += 1;
        if in_loop {
            info.has_assertion_in_loop = true;
        }
        let has_message = args > kind.value_args;
        match kind.style {
            AssertStyle::BooleanOnly => {
                info.assert_only_count += 1;
                if !has_message {
                    info.assert_only_without_message += 1;
                }
            }
            AssertStyle::ShowsValues => {
                if !has_message {
                    info.assertions_without_message += 1;
                }
            }
        }
        collect_magic_numbers(call, source, &mut info.magic_numbers);
        return;
    }

    match method {
        // Swift Testing のアサーション相当
        "record" if callee.starts_with("Issue.") => {
            info.assertion_count += 1;
            info.assert_only_count += 1;
            if in_loop {
                info.has_assertion_in_loop = true;
            }
            return;
        }
        "withKnownIssue" | "confirmation" => {
            info.assertion_count += 1;
            if in_loop {
                info.has_assertion_in_loop = true;
            }
            return;
        }

        // --- スキップ ---
        "XCTSkip" | "XCTSkipIf" | "XCTSkipUnless" => {
            info.has_skip = true;
            return;
        }

        // --- sleep ---
        "sleep" | "usleep" => {
            // Thread.sleep / Task.sleep / usleep / sleep
            info.has_sleep = true;
            return;
        }
        "run" if callee.contains("RunLoop") => {
            info.has_sleep = true;
            info.has_explicit_wait = true;
            return;
        }

        // --- print ---
        "print" | "debugPrint" | "dump" | "NSLog" | "os_log" => {
            info.has_print = true;
            return;
        }

        // --- 待ち・時刻 ---
        "waitForExistence" | "waitForNonExistence" => {
            info.has_explicit_wait = true;
            return;
        }
        "wait" | "waitForExpectations" | "fulfillment" => {
            if has_argument_label(call, source, "timeout") {
                info.has_explicit_wait = true;
            }
            return;
        }
        "now" if callee.starts_with("DispatchTime") || callee.starts_with("Date") => {
            info.has_clock_read = true;
            return;
        }
        "CFAbsoluteTimeGetCurrent" => {
            info.has_clock_read = true;
            return;
        }
        "Date" => {
            // `Date()` — 引数なしのときだけ現在時刻の読み取り
            if args == 0 {
                info.has_clock_read = true;
            }
            return;
        }
        "addingTimeInterval" | "timeIntervalSince" | "timeIntervalSinceNow"
        | "timeIntervalSince1970" | "distance" => {
            info.has_time_arithmetic = true;
            return;
        }
        _ => {}
    }
}

/// XCTAssert 系の分類
struct AssertKind {
    /// メッセージを除いた値引数の数
    value_args: usize,
    style: AssertStyle,
}

enum AssertStyle {
    /// 失敗時に期待値・実際値を表示する（XCTAssertEqual 等）
    ShowsValues,
    /// 真偽値しか分からない（XCTAssertTrue 等）
    BooleanOnly,
}

fn xctest_assertion_kind(method: &str) -> Option<AssertKind> {
    match method {
        "XCTFail" => Some(AssertKind { value_args: 0, style: AssertStyle::BooleanOnly }),
        "XCTAssert" | "XCTAssertTrue" | "XCTAssertFalse" | "XCTAssertNil" | "XCTAssertNotNil"
        | "XCTAssertThrowsError" | "XCTAssertNoThrow" | "XCTUnwrap" => {
            Some(AssertKind { value_args: 1, style: AssertStyle::BooleanOnly })
        }
        "XCTAssertEqual" | "XCTAssertNotEqual" | "XCTAssertIdentical" | "XCTAssertNotIdentical"
        | "XCTAssertGreaterThan" | "XCTAssertGreaterThanOrEqual" | "XCTAssertLessThan"
        | "XCTAssertLessThanOrEqual" => {
            Some(AssertKind { value_args: 2, style: AssertStyle::ShowsValues })
        }
        "XCTAssertEqualWithAccuracy" => {
            Some(AssertKind { value_args: 3, style: AssertStyle::ShowsValues })
        }
        _ => None,
    }
}

/// call_suffix / macro の value_arguments 直下の value_argument 数
fn argument_count(node: &Node) -> usize {
    match find_value_arguments(node) {
        Some(args) => {
            let mut cursor = args.walk();
            args.children(&mut cursor)
                .filter(|n| n.kind() == "value_argument")
                .count()
        }
        None => 0,
    }
}

fn find_value_arguments<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let suffix = find_child(node, "call_suffix")?;
    find_child(&suffix, "value_arguments")
}

/// 指定ラベルの引数があるか（`wait(for:timeout:)` 等）
fn has_argument_label(call: &Node, source: &[u8], label: &str) -> bool {
    let args = match find_value_arguments(call) {
        Some(a) => a,
        None => return false,
    };
    let mut cursor = args.walk();
    let found = args
        .children(&mut cursor)
        .filter(|n| n.kind() == "value_argument")
        .any(|arg| argument_label(&arg, source).as_deref() == Some(label));
    found
}

fn argument_label(arg: &Node, source: &[u8]) -> Option<String> {
    find_child(arg, "value_argument_label").map(|n| node_text(&n, source))
}

/// timeout の秒数は「仕様」ではなく環境のつまみで、ラベル自体が意味を説明しているため
/// マジックナンバーとして扱わない
const NON_MAGIC_LABELS: &[&str] = &[
    "timeout",
    "forTimeInterval",
    "nanoseconds",
    "microseconds",
    "milliseconds",
    "seconds",
    "until",
    "deadline",
];

/// アサーション内の整数リテラルからマジックナンバーを収集する
fn collect_magic_numbers(node: &Node, source: &[u8], out: &mut Vec<(i64, usize)>) {
    // 文字列リテラルの中身と、timeout 系ラベル付き引数は対象外
    if matches!(node.kind(), "line_string_literal" | "multi_line_string_literal" | "raw_string_literal") {
        return;
    }
    if node.kind() == "value_argument" {
        if let Some(label) = argument_label(node, source) {
            if NON_MAGIC_LABELS.contains(&label.as_str()) {
                return;
            }
        }
    }
    if node.kind() == "integer_literal" {
        let text = node_text(node, source).replace('_', "");
        if let Ok(n) = text.parse::<i64>() {
            if !(-10..=10).contains(&n) {
                out.push((n, node.start_position().row + 1));
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_magic_numbers(&child, source, out);
    }
}

fn find_child<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).find(|c| c.kind() == kind);
    found
}

fn node_text(node: &Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<TestFunction> {
        SwiftParser
            .parse("FooTests.swift", source)
            .map(|tf| tf.test_functions)
            .unwrap_or_default()
    }

    #[test]
    fn test_path_detection() {
        let source = "import Testing\n@Test func a() { #expect(true) }\n";
        assert!(SwiftParser.parse("FooTests.swift", source).is_some());
        assert!(SwiftParser.parse("FooTest.swift", source).is_some());
        assert!(SwiftParser.parse("Sources/Tests/Foo.swift", source).is_some());
        assert!(SwiftParser.parse("Sources/Model/Reminder.swift", source).is_none());
    }

    #[test]
    fn test_swift_testing_attribute_not_name_prefix() {
        // @Test が付いていれば test で始まらない名前でも拾う
        let source = r#"
import Testing

@Test("22:00 の毎日リマインダーは 22:01 に催促対象")
func nudgesAfterScheduledTime() {
    #expect(shouldNudge == true)
}

func helperNotATest() {
    #expect(true)
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "nudgesAfterScheduledTime");
        assert!(fns[0].has_assertion);
    }

    #[test]
    fn test_line_points_to_func_not_attribute() {
        let source = r#"
import Testing

@MainActor
@Test("説明")
func something() {
    #expect(true)
}
"#;
        let fns = parse(source);
        assert_eq!(fns[0].line, 6); // func の行（属性の行ではない）
    }

    #[test]
    fn test_nested_in_struct_and_extension() {
        let source = r#"
import Testing

struct SuiteA {
    @Test func inStruct() { #expect(true) }
}

extension SuiteA {
    @Test func inExtension() { #expect(true) }
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
    }

    #[test]
    fn test_xctest_class_methods() {
        let source = r#"
import XCTest

final class LegacyTests: XCTestCase {
    func testAssertions() {
        XCTAssertTrue(flag)
    }

    func helperNotATest() {
        XCTAssertTrue(true)
    }
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "testAssertions");
    }

    #[test]
    fn test_helper_class_in_test_file_is_ignored() {
        // テストファイルに同居するモックのメソッドを巻き込まない
        let source = r#"
import XCTest

final class RealTests: XCTestCase {
    func testReal() { XCTAssertTrue(true) }
}

final class MockURLProtocol: URLProtocol {
    func startLoading() { print("mock") }
    func stopLoading() {}
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "testReal");
    }

    #[test]
    fn test_xctest_convention_requires_xctest_import() {
        // XCTest を使っていないファイルでは test 始まりの func だけでは拾わない
        let source = r#"
import Foundation

final class Helper {
    func testLooksLikeATest() { print("nope") }
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 0);
    }

    #[test]
    fn test_empty_test() {
        let source = r#"
import Testing

@Test func empty() {
}

@Test func notEmpty() { #expect(true) }
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
        assert!(fns[0].is_empty);
        assert!(!fns[1].is_empty);
    }

    #[test]
    fn test_missing_assertion() {
        let source = r#"
import Testing

@Test func noAssertion() {
    let vm = makeViewModel()
    vm.load()
}
"#;
        let fns = parse(source);
        assert!(!fns[0].has_assertion);
    }

    #[test]
    fn test_assertion_variants() {
        let source = r#"
import Testing
import XCTest

@Test func swiftTesting() throws {
    #expect(a == 1)
    let x = try #require(b)
    Issue.record("boom")
}

final class XCTests: XCTestCase {
    func testXCTest() throws {
        XCTAssertEqual(a, b)
        XCTFail("nope")
        let v = try XCTUnwrap(opt)
    }
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].assertion_count, 3);
        assert_eq!(fns[1].assertion_count, 3);
    }

    #[test]
    fn test_assertion_message_classification() {
        let source = r#"
import XCTest

final class T: XCTestCase {
    func testBooleanWithoutMessage() {
        XCTAssertTrue(a)
        XCTAssertTrue(b)
    }

    func testBooleanWithMessage() {
        XCTAssertTrue(a, "a should hold")
        XCTAssertTrue(b, "b should hold")
    }

    func testEqualWithoutMessage() {
        XCTAssertEqual(a, b)
        XCTAssertEqual(c, d)
    }

    func testEqualComparingStringLiteral() {
        XCTAssertEqual(name, "expected")
        XCTAssertEqual(other, "expected")
    }
}
"#;
        let fns = parse(source);
        assert_eq!(fns[0].assert_only_without_message, 2);
        assert_eq!(fns[1].assert_only_without_message, 0);
        assert_eq!(fns[2].assertions_without_message, 2);
        // 第2引数が文字列でも XCTAssertEqual の値引数なのでメッセージ扱いしない
        assert_eq!(fns[3].assertions_without_message, 2);
    }

    #[test]
    fn test_expect_with_comment_counts_as_message() {
        let source = r#"
import Testing

@Test func bare() {
    #expect(a == 1)
    #expect(b == 2)
}

@Test func commented() {
    #expect(a == 1, "a は 1 のはず")
    #expect(b == 2, "b は 2 のはず")
}
"#;
        let fns = parse(source);
        assert_eq!(fns[0].assertions_without_message, 2);
        assert_eq!(fns[1].assertions_without_message, 0);
    }

    #[test]
    fn test_sleepy_variants() {
        let source = r#"
import Testing

@Test func threadSleep() {
    Thread.sleep(forTimeInterval: 0.5)
    #expect(true)
}

@Test func taskSleep() async throws {
    try await Task.sleep(nanoseconds: 500_000_000)
    #expect(true)
}

@Test func uSleep() {
    usleep(1000)
    #expect(true)
}

@Test func noSleep() {
    #expect(true)
}
"#;
        let fns = parse(source);
        assert!(fns[0].has_sleep);
        assert!(fns[1].has_sleep);
        assert!(fns[2].has_sleep);
        assert!(!fns[3].has_sleep);
    }

    #[test]
    fn test_ignored_variants() {
        let source = r#"
import Testing
import XCTest

@Test(.disabled("wip"))
func disabledTest() { #expect(true) }

@Test func activeTest() { #expect(true) }

final class T: XCTestCase {
    func testSkipped() throws {
        throw XCTSkip("not ready")
    }
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 3);
        assert!(fns[0].is_ignored);
        assert!(!fns[1].is_ignored);
        assert!(fns[2].is_ignored);
    }

    #[test]
    fn test_suite_level_disabled() {
        let source = r#"
import Testing

@Suite(.disabled("まとめて止めている"))
struct DisabledSuite {
    @Test func inner() { #expect(true) }
}
"#;
        let fns = parse(source);
        assert_eq!(fns.len(), 1);
        assert!(fns[0].is_ignored);
    }

    #[test]
    fn test_silent_skip_via_guard() {
        let source = r#"
import Testing

@Test func skipsSilently() {
    guard calendar.isDate(oneHourLater, inSameDayAs: Date()) else { return }
    #expect(true)
}

@Test func noSkip() {
    let value = compute()
    #expect(value == 1)
}

@Test func skipsProperly() throws {
    guard let id = makeFixture() else { throw XCTSkip("深夜帯はスキップ") }
    #expect(id.isEmpty == false)
}

@Test func returnInClosureIsNotASkip() {
    let mapped = items.map { item -> Int in
        if item.isEmpty { return 0 }
        return item.count
    }
    #expect(mapped.count == 3)
}
"#;
        let fns = parse(source);
        assert!(fns[0].has_early_return);
        assert!(!fns[1].has_early_return);
        // throw XCTSkip は正しいスキップ（IgnoredTest 側で拾う）
        assert!(!fns[2].has_early_return);
        assert!(fns[2].is_ignored);
        // クロージャ内の return は脱出ではない
        assert!(!fns[3].has_early_return);
    }

    #[test]
    fn test_silent_skip_detected_anywhere_in_body() {
        // 先頭の3文に限らず、本文の後ろにある guard-else-return も拾う
        let source = r#"
import Testing

@Test func lateGuard() {
    let vm = makeViewModel()
    vm.name = "散歩"
    vm.load()
    #expect(vm.isReady)
    guard calendar.isDate(later, inSameDayAs: Date()) else { return }
    #expect(vm.fireAt != nil)
}
"#;
        let fns = parse(source);
        assert!(fns[0].has_early_return);
    }

    #[test]
    fn test_conditional_logic() {
        let source = r#"
import Testing

@Test func withIf() {
    if cond { #expect(a) } else { #expect(b) }
}

@Test func withFor() {
    for item in items { #expect(item.isValid) }
}

@Test func withWhile() {
    while !done { poll() }
    #expect(done)
}

@Test func withSwitch() {
    switch mode {
    case .a: #expect(true)
    default: #expect(false)
    }
}

@Test func withGuardOnly() {
    guard let x = opt else { Issue.record("nil"); return }
    #expect(x == 1)
}
"#;
        let fns = parse(source);
        assert!(fns[0].has_branching);
        assert!(fns[1].has_for_loop);
        assert!(fns[1].has_assertion_in_loop);
        assert!(fns[2].has_conditional);
        assert!(fns[3].has_branching);
        // guard は条件分岐に数えない
        assert!(!fns[4].has_conditional);
    }

    #[test]
    fn test_redundant_print() {
        let source = r#"
import Testing

@Test func printy() {
    print("hi")
    #expect(true)
}

@Test func quiet() {
    #expect(true)
}
"#;
        let fns = parse(source);
        assert!(fns[0].has_print);
        assert!(!fns[1].has_print);
    }

    #[test]
    fn test_magic_numbers() {
        let source = r#"
import Testing

@Test func magic() {
    #expect(calculate() == 12345)
}

@Test func smallNumbers() {
    #expect(count == 3)
}
"#;
        let fns = parse(source);
        assert_eq!(fns[0].magic_numbers.len(), 1);
        assert_eq!(fns[0].magic_numbers[0].0, 12345);
        assert_eq!(fns[1].magic_numbers.len(), 0);
    }

    #[test]
    fn test_timeout_label_is_not_magic_number() {
        let source = r#"
import XCTest

final class UITests: XCTestCase {
    func testWaits() {
        XCTAssertTrue(app.buttons["ok"].waitForExistence(timeout: 30))
    }
}
"#;
        let fns = parse(source);
        assert_eq!(fns[0].magic_numbers.len(), 0);
    }

    #[test]
    fn test_bare_date_is_not_timeout_dependency() {
        let source = r#"
import Testing

@Test func fixtureDates() {
    let r = Reminder(createdAt: Date(), updatedAt: Date())
    #expect(r.isActive)
}

@Test func timeArithmetic() {
    let later = Date().addingTimeInterval(3600)
    #expect(later > Date())
}
"#;
        let fns = parse(source);
        assert!(!fns[0].has_timeout_dependency);
        assert!(fns[1].has_timeout_dependency);
    }

    #[test]
    fn test_xcuitest_flavor() {
        let ui = r#"
import XCTest

final class FlowUITests: XCTestCase {
    func testFlow() {
        let app = XCUIApplication()
        XCTAssertTrue(app.buttons["ok"].exists)
    }
}
"#;
        let tf = SwiftParser.parse("FlowUITests.swift", ui).unwrap();
        assert_eq!(tf.flavor.as_deref(), Some("xcuitest"));

        let unit = "import Testing\n@Test func a() { #expect(true) }\n";
        let tf = SwiftParser.parse("FooTests.swift", unit).unwrap();
        assert_eq!(tf.flavor, None);
    }

    #[test]
    fn test_body_line_count() {
        let source = r#"
import Testing

@Test func short() {
    #expect(true)
}
"#;
        let fns = parse(source);
        assert_eq!(fns[0].body_line_count, 3);
    }
}
