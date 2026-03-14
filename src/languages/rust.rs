use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use syn::{visit::Visit, Attribute, Expr, ExprCall, ExprMacro, ExprPath, ItemFn, ItemMod, Stmt};
use proc_macro2::TokenTree;

pub struct RustParser;

impl LanguageParser for RustParser {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn language_name(&self) -> &str {
        "rust"
    }

    fn parse(&self, path: &str, source: &str) -> Option<TestFile> {
        let syntax = syn::parse_file(source).ok()?;
        let mut visitor = RustTestVisitor::new(source);
        visitor.visit_file(&syntax);

        Some(TestFile {
            path: path.to_string(),
            language: "rust".to_string(),
            test_functions: visitor.test_functions,
            source: Some(source.to_string()),
        })
    }
}

struct RustTestVisitor<'a> {
    source: &'a str,
    test_functions: Vec<TestFunction>,
    in_test_mod: bool,
}

impl<'a> RustTestVisitor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            test_functions: Vec::new(),
            in_test_mod: false,
        }
    }

    fn is_test_attr(attr: &Attribute) -> bool {
        attr.path().is_ident("test")
            || attr.path().segments.last().map_or(false, |s| s.ident == "test")
    }

    fn is_ignore_attr_without_reason(attr: &Attribute) -> bool {
        if !attr.path().is_ident("ignore") {
            return false;
        }
        // #[ignore = "理由"] や #[ignore("理由")] の場合は false（理由付き）
        // #[ignore] のみ true（理由なし）
        attr.meta.require_path_only().is_ok()
    }

    fn has_test_attr(attrs: &[Attribute]) -> bool {
        attrs.iter().any(Self::is_test_attr)
    }

    fn has_ignore_attr(attrs: &[Attribute]) -> bool {
        attrs.iter().any(Self::is_ignore_attr_without_reason)
    }

    fn analyze_function(&self, func: &ItemFn) -> TestFunction {
        let name = func.sig.ident.to_string();
        // ソースコード中の行番号を近似: ident の文字列位置から算出
        let line = self.find_line_of_ident(&name);
        let body_source = self.extract_body_source(&func.block);
        let body_line_count = body_source.lines().count();
        let is_ignored = Self::has_ignore_attr(&func.attrs);
        let is_empty = func.block.stmts.is_empty();

        let mut analyzer = BodyAnalyzer::default();
        // 先頭3文のみ early return チェック（先頭付近の条件付きスキップパターンを検出）
        for stmt in func.block.stmts.iter().take(3) {
            analyzer.check_early_return(stmt);
        }
        for stmt in &func.block.stmts {
            analyzer.visit_stmt(stmt);
        }

        TestFunction {
            name,
            line,
            body_source,
            is_ignored,
            has_assertion: analyzer.has_assertion,
            has_sleep: analyzer.has_sleep,
            has_conditional: analyzer.has_conditional,
            has_branching: analyzer.has_branching,
            has_for_loop: analyzer.has_for_loop,
            has_assertion_in_loop: analyzer.has_assertion_in_loop,
            has_print: analyzer.has_print,
            is_empty,
            assertion_count: analyzer.assertion_count,
            assert_only_count: analyzer.assert_only_count,
            assertions_without_message: analyzer.assertions_without_message,
            assert_only_without_message: analyzer.assert_only_without_message,
            magic_numbers: analyzer.magic_numbers,
            has_early_return: analyzer.has_early_return,
            has_timeout_dependency: analyzer.has_timeout_dependency,
            body_line_count,
        }
    }

    fn find_line_of_ident(&self, name: &str) -> usize {
        // fn <name> を探して行番号を返す
        let pattern = format!("fn {}", name);
        for (i, line) in self.source.lines().enumerate() {
            if line.contains(&pattern) {
                return i + 1;
            }
        }
        1
    }

    fn extract_body_source(&self, block: &syn::Block) -> String {
        // ブロック内の各文を quote で文字列化
        use quote::ToTokens;
        block.to_token_stream().to_string()
    }
}

impl<'a> Visit<'a> for RustTestVisitor<'a> {
    fn visit_item_mod(&mut self, node: &'a ItemMod) {
        // #[cfg(test)] mod tests { ... } に入る
        let is_test_mod = node.attrs.iter().any(|attr| {
            if attr.path().is_ident("cfg") {
                attr.parse_args::<syn::Ident>()
                    .map_or(false, |id| id == "test")
            } else {
                false
            }
        });

        let prev = self.in_test_mod;
        if is_test_mod {
            self.in_test_mod = true;
        }
        syn::visit::visit_item_mod(self, node);
        self.in_test_mod = prev;
    }

    fn visit_item_fn(&mut self, node: &'a ItemFn) {
        if Self::has_test_attr(&node.attrs) {
            let test_fn = self.analyze_function(node);
            self.test_functions.push(test_fn);
        }
        syn::visit::visit_item_fn(self, node);
    }
}

/// 関数ボディの解析
#[derive(Default)]
struct BodyAnalyzer {
    has_assertion: bool,
    has_sleep: bool,
    has_conditional: bool,
    /// 真の条件分岐 (if/match) があるか（for/while/loop は含まない）
    has_branching: bool,
    /// for ループがあるか
    has_for_loop: bool,
    /// for ループ内にアサーションがあるか（テーブル駆動テストの兆候）
    has_assertion_in_loop: bool,
    /// 現在ループの中にいるか（再帰的追跡用）
    in_loop: bool,
    has_print: bool,
    assertion_count: usize,
    /// assert! のみのカウント（assert_eq!/assert_ne! を除く）
    assert_only_count: usize,
    /// メッセージなし assert_eq!/assert_ne!/debug_assert_eq!/debug_assert_ne! のカウント
    assertions_without_message: usize,
    /// メッセージなし assert!/debug_assert! のカウント
    assert_only_without_message: usize,
    magic_numbers: Vec<(i64, usize)>,
    has_early_return: bool,
    /// Duration::from_secs/from_millis, Instant::now(), SystemTime::now() などの時間API使用（sleep以外）
    has_timeout_dependency: bool,
}

impl BodyAnalyzer {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(expr, _) => {
                self.visit_expr(expr);
            }
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.visit_expr(&init.expr);
                }
            }
            Stmt::Macro(stmt_macro) => {
                self.check_macro_path(&stmt_macro.mac);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Macro(ExprMacro { mac, .. }) => {
                self.check_macro_path(mac);
            }
            Expr::Call(ExprCall { func, .. }) => {
                if let Expr::Path(ExprPath { path, .. }) = func.as_ref() {
                    self.check_fn_call(path);
                }
            }
            Expr::If(expr_if) => {
                self.has_conditional = true;
                self.has_branching = true;
                for stmt in &expr_if.then_branch.stmts {
                    self.visit_stmt(stmt);
                }
                if let Some((_, else_branch)) = &expr_if.else_branch {
                    self.visit_expr(else_branch);
                }
            }
            Expr::Match(expr_match) => {
                self.has_conditional = true;
                self.has_branching = true;
                for arm in &expr_match.arms {
                    self.visit_expr(&arm.body);
                }
            }
            Expr::ForLoop(expr_for) => {
                self.has_conditional = true;
                self.has_for_loop = true;
                let prev_in_loop = self.in_loop;
                self.in_loop = true;
                for stmt in &expr_for.body.stmts {
                    self.visit_stmt(stmt);
                }
                self.in_loop = prev_in_loop;
            }
            Expr::While(expr_while) => {
                self.has_conditional = true;
                let prev_in_loop = self.in_loop;
                self.in_loop = true;
                for stmt in &expr_while.body.stmts {
                    self.visit_stmt(stmt);
                }
                self.in_loop = prev_in_loop;
            }
            Expr::Loop(expr_loop) => {
                self.has_conditional = true;
                let prev_in_loop = self.in_loop;
                self.in_loop = true;
                for stmt in &expr_loop.body.stmts {
                    self.visit_stmt(stmt);
                }
                self.in_loop = prev_in_loop;
            }
            Expr::Block(block) => {
                for stmt in &block.block.stmts {
                    self.visit_stmt(stmt);
                }
            }
            Expr::MethodCall(method) => {
                let method_name = method.method.to_string();
                if method_name == "sleep" {
                    self.has_sleep = true;
                }
                // Recurse into receiver
                self.visit_expr(&method.receiver);
            }
            _ => {}
        }
    }

    fn extract_magic_numbers_from_macro(&mut self, mac: &syn::Macro) {
        let path_str = macro_path_string(&mac.path);
        if !matches!(path_str.as_str(), "assert_eq" | "assert_ne" | "assert") {
            return;
        }
        // トークンストリームから数値リテラルを抽出
        for token in mac.tokens.clone() {
            if let TokenTree::Literal(lit) = token {
                let s = lit.to_string();
                if let Ok(n) = s.parse::<i64>() {
                    // 小さい数値（-10..=10）は一般的な境界値・定数なので除外
                    if !(-10..=10).contains(&n) {
                        self.magic_numbers.push((n, 0));
                    }
                }
            }
        }
    }

    fn check_macro_path(&mut self, mac: &syn::Macro) {
        let path_str = macro_path_string(&mac.path);
        // Assertion macros
        if matches!(
            path_str.as_str(),
            "assert" | "assert_eq" | "assert_ne"
                | "debug_assert" | "debug_assert_eq" | "debug_assert_ne"
                | "panic"
        ) {
            self.has_assertion = true;
            self.assertion_count += 1;
            if self.in_loop {
                self.has_assertion_in_loop = true;
            }
        }
        // assert! のみのカウント（assert_eq!/assert_ne! を除く）
        if matches!(path_str.as_str(), "assert" | "debug_assert") {
            self.assert_only_count += 1;
        }

        // メッセージなしアサーションのカウント
        // assert!/debug_assert!: カンマが1つ以上あればメッセージ付き
        // assert_eq!/assert_ne!/debug_assert_eq!/debug_assert_ne!: カンマが2つ以上あればメッセージ付き
        if matches!(path_str.as_str(), "assert" | "debug_assert") {
            let comma_count = count_commas(&mac.tokens);
            if comma_count == 0 {
                self.assert_only_without_message += 1;
            }
        }
        if matches!(
            path_str.as_str(),
            "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne"
        ) {
            let comma_count = count_commas(&mac.tokens);
            if comma_count < 2 {
                self.assertions_without_message += 1;
            }
        }

        // Print macros
        if matches!(
            path_str.as_str(),
            "print" | "println" | "eprint" | "eprintln" | "dbg"
        ) {
            self.has_print = true;
        }
        // Magic number extraction
        self.extract_magic_numbers_from_macro(mac);
    }

    /// テスト先頭付近の条件付き early return を検出する
    /// `if <cond> { return; }` または `if <cond> { return <expr>; }` パターン
    fn check_early_return(&mut self, stmt: &Stmt) {
        if let Stmt::Expr(Expr::If(expr_if), _) = stmt {
            // then_branch に return があるか確認
            let has_return = expr_if.then_branch.stmts.iter().any(|s| {
                matches!(s, Stmt::Expr(Expr::Return(_), _))
            });
            if has_return {
                self.has_early_return = true;
            }
        }
    }

    fn check_fn_call(&mut self, path: &syn::Path) {
        let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let last = segments.last().map(|s| s.as_str());
        let full = segments.join("::");

        if let Some(name) = last {
            if name == "sleep" {
                self.has_sleep = true;
            }
        }

        // 時間API検出: Duration::from_secs, Duration::from_millis, Instant::now, SystemTime::now
        let timeout_fns = [
            "Duration::from_secs",
            "Duration::from_millis",
            "Duration::from_micros",
            "Duration::from_nanos",
            "Instant::now",
            "SystemTime::now",
        ];
        if timeout_fns.iter().any(|&f| full.ends_with(f)) {
            self.has_timeout_dependency = true;
        }
    }
}

fn macro_path_string(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

/// マクロのトークンストリームにおけるトップレベルのカンマ数を数える
fn count_commas(tokens: &proc_macro2::TokenStream) -> usize {
    tokens
        .clone()
        .into_iter()
        .filter(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ','))
        .count()
}
