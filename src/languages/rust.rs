use crate::core::{TestFile, TestFunction};
use super::LanguageParser;
use syn::{visit::Visit, Attribute, Expr, ExprCall, ExprMacro, ExprPath, ItemFn, ItemMod, Stmt};

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

    fn is_ignore_attr(attr: &Attribute) -> bool {
        attr.path().is_ident("ignore")
    }

    fn has_test_attr(attrs: &[Attribute]) -> bool {
        attrs.iter().any(Self::is_test_attr)
    }

    fn has_ignore_attr(attrs: &[Attribute]) -> bool {
        attrs.iter().any(Self::is_ignore_attr)
    }

    fn analyze_function(&self, func: &ItemFn) -> TestFunction {
        let name = func.sig.ident.to_string();
        // ソースコード中の行番号を近似: ident の文字列位置から算出
        let line = self.find_line_of_ident(&name);
        let body_source = self.extract_body_source(&func.block);
        let is_ignored = Self::has_ignore_attr(&func.attrs);
        let is_empty = func.block.stmts.is_empty();

        let mut analyzer = BodyAnalyzer::default();
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
            has_print: analyzer.has_print,
            is_empty,
            assertion_count: analyzer.assertion_count,
            magic_numbers: Vec::new(), // TODO: Phase 2
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
    has_print: bool,
    assertion_count: usize,
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
                for stmt in &expr_if.then_branch.stmts {
                    self.visit_stmt(stmt);
                }
                if let Some((_, else_branch)) = &expr_if.else_branch {
                    self.visit_expr(else_branch);
                }
            }
            Expr::Match(expr_match) => {
                self.has_conditional = true;
                for arm in &expr_match.arms {
                    self.visit_expr(&arm.body);
                }
            }
            Expr::ForLoop(expr_for) => {
                self.has_conditional = true;
                for stmt in &expr_for.body.stmts {
                    self.visit_stmt(stmt);
                }
            }
            Expr::While(expr_while) => {
                self.has_conditional = true;
                for stmt in &expr_while.body.stmts {
                    self.visit_stmt(stmt);
                }
            }
            Expr::Loop(expr_loop) => {
                self.has_conditional = true;
                for stmt in &expr_loop.body.stmts {
                    self.visit_stmt(stmt);
                }
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
        }
        // Print macros
        if matches!(
            path_str.as_str(),
            "print" | "println" | "eprint" | "eprintln" | "dbg"
        ) {
            self.has_print = true;
        }
    }

    fn check_fn_call(&mut self, path: &syn::Path) {
        let last = path.segments.last().map(|s| s.ident.to_string());
        if let Some(name) = last {
            if name == "sleep" {
                self.has_sleep = true;
            }
        }
    }
}

fn macro_path_string(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}
