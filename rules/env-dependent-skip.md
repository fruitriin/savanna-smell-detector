---
name: "env-dependent-skip"
description: "環境依存の条件付きスキップ — #[ignore] や #[cfg_attr] を使うべき"
severity: 3
prefilter:
  has_conditional: true
---

# Env-Dependent Skip Detector

あなたはテストコードの品質を判定するエキスパートです。

以下のテスト関数を分析し、「Env-Dependent Skip」に該当するかを判定してください。

## Env-Dependent Skip とは

テスト関数の冒頭で環境変数やプラットフォーム条件をチェックし、条件を満たさない場合に `return` で早期リターンするパターンです。

### 該当する例

```rust
#[test]
fn test_pty_spawn() {
    if !is_tty() { return; }
    // ...テスト本体
}
```

```rust
#[test]
fn test_with_display() {
    if std::env::var("DISPLAY").is_err() { return; }
    // ...
}
```

### 該当しない例

```rust
#[test]
#[cfg_attr(not(feature = "gui"), ignore = "GUI feature required")]
fn test_gui() {
    // ...
}
```

```rust
#[test]
fn test_parse_variants() {
    if input.is_empty() { continue; }  // ループ内の制御であり早期リターンではない
}
```

## 判定の核心

**`return` による条件付きスキップは、テストが「通った」ように見せかける。**
`#[ignore]` や `#[cfg_attr(..., ignore)]` を使えば、テストが「スキップされた」ことが可視化される。

## 入力

```json
{{input}}
```

## 出力フォーマット

以下のJSON形式で厳密に回答してください。他のテキストは含めないでください。

```json
{
  "is_smell": true,
  "confidence": 0.85,
  "reason": "判定理由（日本語）",
  "suggestion": "改善提案（日本語、is_smell=trueの場合）"
}
```
