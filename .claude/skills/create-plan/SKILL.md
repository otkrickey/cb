---
name: create-plan
description: 実装計画を新規作成する。新機能の実装やバグ修正の計画を立てる際に使用。引数に実装テーマを指定。
allowed-tools: Read, Write, Glob, Grep, AskUserQuestion
---

# 実装計画作成

`docs/plans/` に新しい実装計画を追加する。

## 手順

1. `docs/plans/` と `docs/plans/resolved/` を Glob し、既存の最大番号を取得する
2. `docs/plans/GUIDE.md` を Read し、記法規約を確認する
3. 引数のテーマに基づき、コードベースを調査してスコープを把握する
4. AskUserQuestion で以下を確認する:
   - 種別（enhancement / bugfix / refactoring / missing_docs）
   - 優先度（高 / 中 / 低）
   - スコープの確認（対応範囲と対応外）
5. `docs/plans/TEMPLATE.md` を Read し、テンプレートに基づいて計画を作成する
6. 種別に応じて不要セクションを削除する（GUIDE.md の推奨セクション表を参照）

## タスク設計のルール

- 1タスク = 1-3時間で完了可能なサイズに分割
- タスクID: `#{ファイル番号}-{2桁連番}`（例: `#003-01`）
- 依存関係を明示し、並列実行可能なタスクを特定する
- 各タスクに対象ファイルと実装内容を具体的に記載する

## 命名規則

`{3桁番号}-{kebab-case}.md`（例: `002-clipboard-monitoring.md`）

## 出力

作成した実装計画のファイルパスとタスク一覧のサマリーを報告する。
