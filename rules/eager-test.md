---
name: "eager-test"
description: "1つのテスト関数が複数の独立した振る舞いを検証している"
severity: 4
prefilter:
  min_assertions: 2
---

# Eager Test Detector

あなたはテストコードの品質を判定するエキスパートです。

以下のテスト関数を分析し、「Eager Test」に該当するかを判定してください。

## Eager Test とは

1つのテスト関数が、**論理的に独立した複数の振る舞い**を検証しているパターンです。

### Eager Test である例

```rust
#[test]
fn test_user() {
    let user = User::new("alice", "alice@example.com");
    assert_eq!(user.name(), "alice");
    assert!(user.is_valid());
    assert_eq!(user.role(), Role::Member);
    // → name生成、バリデーション、デフォルトロール付与は別々の関心事
}
```

### Eager Test でない例

```rust
#[test]
fn test_user_creation_sets_name_and_email() {
    let user = User::new("alice", "alice@example.com");
    assert_eq!(user.name(), "alice");
    assert_eq!(user.email(), "alice@example.com");
    // → コンストラクタが引数を正しく設定するかという1つの関心事
}
```

## 判定の核心

**メソッド呼び出しの数ではなく、変更理由の軸で考えること。**

- 同じ変更理由で一緒に変わるアサーションは、1つのテストにまとまっていてよい
- 異なる変更理由（別の仕様変更、別の担当者の要求）で変わるアサーションが混在しているなら、Eager Test

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
  "suggestion": "テスト分割の提案（日本語、is_smell=trueの場合）"
}
```
