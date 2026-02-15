---
name: execute-review
description: 指定モジュールのレビューを実行する。サブエージェント内で使用。引数にモジュール名（core/wasm/server/web/docs）を指定。
allowed-tools: Read, Grep, Glob, Bash(git diff*), Bash(git log*)
---

# モジュールレビュー実行

引数で指定されたモジュールの diff を `docs/review/modules/{module}.md` の基準でレビューする。

## パスマッピング

| モジュール | git diff パス | 依存先 |
|-----------|--------------|--------|
| rust | `crates/**` | — |
| swift | `Sources/**` `cb/**` `cb.xcodeproj/**` `Package.swift` | rust |
| docs | `docs/**` `CLAUDE.md` | rust, swift |

**依存先**: そのモジュールのレビュー基準が参照する他モジュール。依存先が変更された場合、再レビューが必要になる可能性がある。

## 分類基準

指摘を以下の3段階に分類する。分類はレビュー分析中に行い、出力時に振り分ける。

| レベル | 基準 | 対応 |
|--------|------|------|
| **Critical** | マージ前に必ず修正が必要。正しさ・安全性・設計整合性に影響する問題 | 必須修正 |
| **Medium** | 修正推奨。品質・保守性・堅牢性が向上する改善点 | 推奨修正 |
| **Low** | 将来対応の提案。現時点でマージを妨げない | 記録・先送り可 |

## 手順

1. `git diff main...HEAD --name-only -- {paths}` で変更検出。変更なし → 報告して終了
2. `git diff main...HEAD -- {paths}` で diff 取得
3. `docs/review/modules/{module}.md` を Read し、評価項目を確認
4. 変更ファイルを Read してコンテキスト把握。設計書への参照がある場合はその内容も読む
5. 評価項目に基づいてレビューし、各指摘を上記の分類基準で Critical / Medium / Low に分類する
6. `docs/review/FORMAT.md` を Read し、出力形式に従って結果を出力

チームエージェントの場合は SendMessage でリーダーに報告する。
