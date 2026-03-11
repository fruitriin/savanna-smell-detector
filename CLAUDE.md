# CLAUDE.md

このファイルは、このリポジトリでコードを扱う際に Claude Code にガイダンスを提供します。

## プロジェクト概要

savanna-smell-detector — テストコードのアンチパターン（テスト臭い）を AST 解析で検出する CLI ツール。
原作: [savanna-maven-plugin](https://github.com/kawasima/savanna-maven-plugin) by @kawasima

## ビルド・実行

```bash
# ビルド
cargo build

# 実行
cargo run -- <対象パス>
cargo run -- . -f json          # JSON出力
cargo run -- . --fail-on-smell  # スメル検出時に非ゼロ終了

# テスト
cargo test
```

## アーキテクチャ

```
src/
├── main.rs              # CLI エントリポイント (clap)
├── core/                # 言語非依存のコアモデル
│   ├── smell.rs         # SmellType, TestSmell, TestFunction, TestFile
│   ├── detector.rs      # SmellDetector trait
│   └── registry.rs      # SmellDetectorRegistry (enable/disable)
├── detectors/           # スメル検出器（ここに追加していく）
│   ├── empty_test.rs
│   ├── missing_assertion.rs
│   ├── sleepy_test.rs
│   ├── conditional_logic.rs
│   ├── ignored_test.rs
│   └── redundant_print.rs
├── languages/           # 言語パーサー（拡張ポイント）
│   └── rust.rs          # syn による Rust AST 解析
└── reporters/           # 出力形式
    ├── console.rs       # 色付きコンソール + 🦁
    └── json.rs          # CI/LLM連携用 JSON
```

## 設計原則

- **多言語対応**: `LanguageParser` trait を実装すれば新しい言語を追加できる
- **検出器の独立性**: 各 `SmellDetector` は独立して動作し、`TestFile`（言語非依存の中間表現）を受け取る
- **CI ファースト**: JSON 出力 + `--fail-on-smell` で CI パイプラインに組み込める
- **LLM フレンドリー**: 構造化された JSON 出力を LLM が読んで自動修正できることを想定

## 新しいスメルの追加手順

1. `src/core/smell.rs` の `SmellType` に variant を追加し、`roar()` と `severity()` を実装
2. `src/detectors/` に新しい検出器ファイルを作成し、`SmellDetector` trait を実装
3. `src/detectors/mod.rs` で公開し、`default_registry()` に登録
4. `tests/fixtures/` にテストフィクスチャを追加
5. `cargo build` が通ることを確認

### t_wada テスト

スメルを追加する判断基準: **t_wada がそのテストコードを見て眉をひそめるか？**
眉をひそめるなら、それはスメルだ。

## 新しい言語パーサーの追加手順

1. `src/languages/` に新しいファイルを作成し、`LanguageParser` trait を実装
2. `parse()` メソッドでソースコードを解析し、`TestFile`（テスト関数のリスト）を返す
3. `src/languages/mod.rs` で公開
4. `src/main.rs` の `parsers` ベクタに登録

## コントリビューションモデル

- コードではなく計画（Plan）をレビューする。筋の良い計画は受け入れ、実装はAIが担保する
- 派生Forkの改善は積極的にアップストリームへ取り込む（Apache-2.0）
- CLAUDE.md を改善した場合は、CONTRIBUTING.md を読んでオーナーへの確認を必ず行うこと
- 詳細は [CONTRIBUTING.md](./CONTRIBUTING.md) を参照
