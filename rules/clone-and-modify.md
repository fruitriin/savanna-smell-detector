---
name: "clone-and-modify"
description: "テスト間のコピペコード — ヘルパー関数やパラメタライズドテストに統合すべき"
severity: 2
prefilter:
  min_lines: 5
---

# Clone-and-Modify Test Detector

あなたはテストコードの品質を判定するエキスパートです。

以下のテスト関数を分析し、同一ファイル内の他のテスト関数と構造的に重複していないかを判定してください。

## Clone-and-Modify Test とは

隣接するテスト関数のコードが80%以上類似しており、わずかな入力値やアサーション値だけが異なるパターンです。

### 該当する例

```rust
#[test]
fn test_parse_integer() {
    let input = "42";
    let result = parse(input);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value(), 42);
}

#[test]
fn test_parse_negative() {
    let input = "-7";
    let result = parse(input);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().value(), -7);
}
```

→ パラメタライズドテスト（`#[test_case]` や `rstest`）やテーブル駆動テストに統合すべき。

### 該当しない例

```rust
#[test]
fn test_parse_integer() {
    let result = parse("42");
    assert_eq!(result.unwrap().value(), 42);
}

#[test]
fn test_parse_error_handling() {
    let result = parse("not_a_number");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
}
```

→ 正常系と異常系は異なるロジックパスを検証しており、統合すべきではない。

## 判定の核心

**テストコードの重複は、片方を直してもう片方を直し忘れる未来への招待状です。**
ただし、テスト間でコンテキスト（正常系/異常系、境界値/通常値）が異なる場合は、重複ではなく意図的な分離です。

## 入力

```json
{{input}}
```

## 出力フォーマット

以下のJSON形式で厳密に回答してください。他のテキストは含めないでください。

```json
{
  "is_smell": true,
  "confidence": 0.80,
  "reason": "判定理由（日本語）",
  "suggestion": "改善提案（日本語、is_smell=trueの場合）"
}
```
