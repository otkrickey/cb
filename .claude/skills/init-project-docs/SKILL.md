---
name: init-project-docs
description: 新規プロジェクトのドキュメント体系を一括セットアップする。docs/配下のディレクトリ構造、GUIDE.md、TEMPLATE.md、初期ADR、ステータス文書、初期仕様書を生成する。プロジェクトの新規立ち上げ時に使用。
allowed-tools: Read, Write, Edit, Glob, Grep, Bash(mkdir*), AskUserQuestion
---

# プロジェクトドキュメント初期セットアップ

新規プロジェクトの `docs/` ディレクトリ体系を構築する。

## 前提

- ドキュメント設計パターンは [docs-directory-guide](DOCS_PATTERN.md) に準拠する
- 以下の参照プロジェクトの `docs/` 構造を分析し、同等の体系を構築する

## 参照プロジェクト

| プロジェクト | パス | 特徴 |
|------------|------|------|
| **qs**（量子将棋） | `~/dev/qs` | フル構成の参照実装。Rust + React + WASM。design/, plans/, review/, usecases/, status/ の全カテゴリを網羅。ADR 8件、実装計画16件、モジュール設計書9件 |
| **cb**（クリップボードマネージャー） | `~/dev/cb` | 本スキルで構築した実例。Swift + Rust ハイブリッド。Liquid Glass デザイン。フル構成（issues/除く） |

新規プロジェクトでは、これらの `docs/` 構造を Glob で走査して構造・命名規則・セクション構成を参考にする。特に `qs` は最も成熟した参照実装であり、GUIDE.md やTEMPLATE.md の品質基準として活用する。

## ワークフロー

```
進捗:
- [ ] Step 1: プロジェクト情報収集
- [ ] Step 2: ディレクトリ構造作成
- [ ] Step 3: GUIDE.md・TEMPLATE.md 生成
- [ ] Step 4: 初期仕様書作成
- [ ] Step 5: 初期ADR作成
- [ ] Step 6: ステータス文書作成
- [ ] Step 7: 品質チェック
```

### Step 1: プロジェクト情報収集

AskUserQuestion で以下を確認する:

1. **プロジェクト名と概要**: 何を作るか
2. **技術スタック**: 言語・フレームワーク・DB等
3. **配布方法**: App Store / GitHub Release / npm等
4. **参照プロジェクト**: デフォルトで `~/dev/qs`（フル構成）と `~/dev/cb`（ハイブリッド構成）を参照。追加の参照プロジェクトがあればパスを指定
5. **必要なカテゴリ**: 規模に応じて選択

カテゴリ選択の目安:

| 規模 | カテゴリ |
|------|---------|
| 最小 | design/decisions/, archive/ |
| 標準 | + design/modules/, design/flows/, usecases/, status/ |
| フル | + plans/, review/, design/usecase_mapping/ |

参照プロジェクトが指定された場合は `docs/` を Glob で走査し、構造を分析する。

### Step 2: ディレクトリ構造作成

`mkdir -p` で選択されたカテゴリのディレクトリを一括作成する。

```bash
mkdir -p docs/{archive,design/{decisions,modules,flows},plans/resolved,review/modules,usecases,status}
```

不要なカテゴリは除外する。

### Step 3: GUIDE.md・TEMPLATE.md 生成

各カテゴリに GUIDE.md と TEMPLATE.md を生成する。[TEMPLATES.md](TEMPLATES.md) を Read し、プロジェクト固有の情報で埋める。

**生成ルール**:
- プロジェクト名・技術スタック・対象読者をテンプレートに反映する
- コード参照形式はプロジェクトの言語に合わせる（Swift / Rust / TypeScript 等）
- 空セクションを残さない

### Step 4: 初期仕様書作成

`docs/archive/initial_plan.md` にプロジェクトの初期仕様を記録する。

含める内容:
- プロジェクト概要・動機
- コア機能一覧
- アーキテクチャ概要
- 技術要件
- ターゲットユーザー

### Step 5: 初期ADR作成

最低限の ADR を作成する。[create-adr スキル](../create-adr/SKILL.md) の手順に従う。

必須ADR:
- `001-technology-stack.md` — 技術スタック選定（言語・フレームワーク・DB・ビルド・配布）

任意ADR（プロジェクトに応じて）:
- UIデザインシステム
- アーキテクチャパターン
- データモデル方針

### Step 6: ステータス文書作成

`docs/status/` に以下を生成する:
- `implementation.md` — モジュール別実装ステータス（全て `not-started`）
- `roadmap.md` — Phase 1-3 のロードマップ

### Step 7: 品質チェック

全生成物に対して確認する:

- [ ] 各カテゴリに GUIDE.md が存在する
- [ ] TEMPLATE.md が存在する（該当カテゴリ）
- [ ] 相互参照の相対パスが正しい
- [ ] メタデータコメントが記入されている（ADR）
- [ ] 空セクション・空ファイルがない
- [ ] プロジェクト固有の情報が正しく反映されている

## 出力

最終的に生成されたファイル一覧をツリー形式で報告する。
