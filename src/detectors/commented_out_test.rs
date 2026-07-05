use crate::core::{SmellDetector, SmellType, TestFile, TestSmell};
use regex::Regex;

pub struct CommentedOutTestDetector;

impl SmellDetector for CommentedOutTestDetector {
    fn name(&self) -> &'static str {
        "CommentedOutTest"
    }

    fn detect(&self, test_file: &TestFile) -> Vec<TestSmell> {
        let source = match &test_file.source {
            Some(s) => s,
            None => return Vec::new(),
        };

        match test_file.language.as_str() {
            "rust" => detect_rust(source, &test_file.path),
            "go" => detect_go(source, &test_file.path),
            "shell" => detect_shell(source, &test_file.path),
            "python" => detect_python(source, &test_file.path),
            _ => Vec::new(),
        }
    }
}

fn detect_rust(source: &str, path: &str) -> Vec<TestSmell> {
    let mut results = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        // // #[test] パターン
        if trimmed.starts_with("//") && trimmed.contains("#[test]") {
            results.push(TestSmell::new(
                SmellType::CommentedOutTest,
                path,
                i + 1,
                None,
            ));
        }
        // /* ... #[test] ... */ パターン
        if trimmed.starts_with("/*") && trimmed.contains("#[test]") {
            results.push(TestSmell::new(
                SmellType::CommentedOutTest,
                path,
                i + 1,
                None,
            ));
        }
    }
    results
}

fn detect_go(source: &str, path: &str) -> Vec<TestSmell> {
    let pattern = Regex::new(r"^\s*//\s*func\s+(Test|Benchmark|Fuzz|Example)\w+\s*\(").unwrap();
    let mut results = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if pattern.is_match(line) {
            results.push(TestSmell::new(
                SmellType::CommentedOutTest,
                path,
                i + 1,
                None,
            ));
        }
    }
    results
}

fn detect_shell(source: &str, path: &str) -> Vec<TestSmell> {
    let pattern_bats = Regex::new(r"^\s*#\s*@test\s+").unwrap();
    let pattern_sh = Regex::new(r"^\s*#\s*(test\w+\s*\(\)|function\s+test\w+)").unwrap();
    let mut results = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if pattern_bats.is_match(line) || pattern_sh.is_match(line) {
            results.push(TestSmell::new(
                SmellType::CommentedOutTest,
                path,
                i + 1,
                None,
            ));
        }
    }
    results
}

fn detect_python(source: &str, path: &str) -> Vec<TestSmell> {
    let pattern = Regex::new(r"^\s*#\s*(async\s+)?def\s+test\w*\s*\(").unwrap();
    let mut results = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if pattern.is_match(line) {
            results.push(TestSmell::new(
                SmellType::CommentedOutTest,
                path,
                i + 1,
                None,
            ));
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TestFile;

    fn make_test_file(language: &str, source: &str) -> TestFile {
        TestFile {
            path: "test_file".to_string(),
            language: language.to_string(),
            test_functions: Vec::new(),
            source: Some(source.to_string()),
        }
    }

    #[test]
    fn test_detect_rust_commented_test() {
        let source = r#"
#[cfg(test)]
mod tests {
    // #[test]
    // fn test_something() {
    //     assert_eq!(1, 1);
    // }

    #[test]
    fn test_real() {
        assert_eq!(1, 1);
    }
}
"#;
        let tf = make_test_file("rust", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 1);
        assert_eq!(smells[0].line, 4);
    }

    #[test]
    fn test_detect_rust_block_comment() {
        let source = r#"
/* #[test]
fn test_commented_block() {} */
"#;
        let tf = make_test_file("rust", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 1);
    }

    #[test]
    fn test_detect_go_commented_test() {
        let source = r#"
package foo_test

import "testing"

// func TestCommentedOut(t *testing.T) {
//     t.Error("this test is commented out")
// }

func TestReal(t *testing.T) {
    t.Log("ok")
}
"#;
        let tf = make_test_file("go", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 1);
        assert_eq!(smells[0].line, 6);
    }

    #[test]
    fn test_detect_go_benchmark_and_fuzz() {
        let source = r#"
// func BenchmarkFoo(b *testing.B) {}
// func FuzzBar(f *testing.F) {}
// func ExampleBaz() {}
"#;
        let tf = make_test_file("go", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 3);
    }

    #[test]
    fn test_detect_shell_bats_commented_test() {
        let source = r#"
#!/usr/bin/env bats

@test "real test" {
    [ 1 -eq 1 ]
}

# @test "commented out test" {
#     [ 1 -eq 1 ]
# }
"#;
        let tf = make_test_file("shell", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 1);
        assert_eq!(smells[0].line, 8);
    }

    #[test]
    fn test_detect_shell_sh_commented_test() {
        let source = r#"
#!/bin/bash

test_real() {
    assertEquals 1 1
}

# test_commented_out() {
#     assertEquals 1 1
# }

# function test_function_commented {
#     assertEquals 1 1
# }
"#;
        let tf = make_test_file("shell", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 2);
    }

    #[test]
    fn test_no_false_positive_in_comments() {
        // 通常のコメント行（テスト関数ではない）を誤検知しないこと
        let source_go = r#"
// This is a regular comment about tests
// func helper() {} // not a test function
"#;
        let tf = make_test_file("go", source_go);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 0);

        // 言語が未知の場合は何も返さない
        let source_unknown = r#"
// #[test]
fn something() {}
"#;
        let tf_unknown = make_test_file("ruby", source_unknown);
        let smells_unknown = detector.detect(&tf_unknown);
        assert_eq!(smells_unknown.len(), 0);
    }

    #[test]
    fn test_detect_python_commented_test() {
        let source = r#"
import unittest

def test_real():
    assert True

# def test_commented_out():
#     assert 1 == 1

# async def test_async_commented():
#     assert True

# コメント内で test という単語に触れているだけの行
# this line just mentions def and test but no function
"#;
        let tf = make_test_file("python", source);
        let detector = CommentedOutTestDetector;
        let smells = detector.detect(&tf);
        assert_eq!(smells.len(), 2);
        assert_eq!(smells[0].line, 7);
        assert_eq!(smells[1].line, 10);
    }
}
