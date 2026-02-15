# docs ディレクトリ設計ガイド

nob2mdプロジェクトの`docs/`ディレクトリ構成から抽出した、プロジェクトドキュメント管理のパターン集。
本ガイドはdocsディレクトリ全体の設計方針を扱う。

### 関連ガイド

本ガイドは以下の2つのガイドを前提知識とする。docs/ 内のドキュメントはすべてAIエージェントが読み書きする対象であり、LLMフレンドリーな記法を全体に適用すべきである。

| ガイド | 本ガイドとの関係 |
|-------|----------------|
| [claude-md-writing-guide.md](claude-md-writing-guide.md) | CLAUDE.md だけでなく、docs/ 内の全ドキュメントに同じ原則（簡潔さ、自由度設計、段階的開示、テンプレート・例示）を適用する |
| [agent-skills-best-practices.md](agent-skills-best-practices.md) | docs/ 内のユースケース・ガイド・テンプレートの設計指針（ワークフロー、フィードバックループ、条件分岐、用語統一）として活用する |

### LLMフレンドリーな記法の原則

docs/ 内の全ドキュメントに適用する横断的ルール:

- **簡潔さ**: プロジェクト固有の情報のみ記載する（→ [claude-md-writing-guide.md 1章](claude-md-writing-guide.md)）
- **自由度の設計**: 高リスク操作は具体的に、低リスク操作はAIエージェントの裁量に委ねる（→ [claude-md-writing-guide.md 2章](claude-md-writing-guide.md)）
- **段階的開示**: GUIDE.md は概要とナビゲーションに徹し、詳細は別ファイルへ。参照は1レベルまで（→ [claude-md-writing-guide.md 3章](claude-md-writing-guide.md)）
- **テンプレートと例示**: 一貫性が必要な場合はテンプレートを提供し、入出力例で期待品質を示す（→ [claude-md-writing-guide.md 4章](claude-md-writing-guide.md)）
- **ワークフローとフィードバックループ**: 複数ステップの操作にはチェックリスト、検証→修正→再試行ループを含める（→ [agent-skills-best-practices.md](agent-skills-best-practices.md)）
- **一貫した用語**: ドキュメント全体で用語を統一し、同義語の混在を避ける（→ [claude-md-writing-guide.md 7章](claude-md-writing-guide.md)）

---

## 1. 全体構成

```
docs/
├── COMMANDS.md                  # CLIコマンドリファレンス
├── DEVELOPMENT.md               # 開発者向けガイド
├── design/                      # 設計ドキュメント（実装に基づく）
│   ├── GUIDE.md
│   ├── TEMPLATE.md
│   ├── modules/                 # モジュール別設計書
│   ├── flows/                   # 処理フロー図
│   ├── decisions/               # アーキテクチャ決定記録（ADR）
│   └── usecase_mapping/         # ユースケースと実装の対応表
├── usecases/                    # ユーザー向け操作ドキュメント
│   ├── GUIDE.md
│   ├── TEMPLATE.md
│   └── {command}.md
├── issues/                      # バグ・問題の追跡（GitHub Issues に移行済み）
│   └── GUIDE.md                 # 移行ガイド・旧ID対応表
├── plans/         # 実装計画
│   ├── GUIDE.md
│   ├── TEMPLATE.md
│   └── resolved/                # 完了済み
└── archive/                     # 歴史的ドキュメント（参照のみ）
```

| レイヤー | 問いに答える |
|---------|------------|
| **ルート** (`COMMANDS.md`, `DEVELOPMENT.md`) | 「何ができるか」「どうビルドするか」 |
| **design/** | 「なぜこう作られたか」「内部でどう動くか」 |
| **usecases/** | 「どう使うか」 |
| **issues/** | 「何が壊れたか」「どう直すか」（GitHub Issues に移行済み） |
| **plans/** | 「次に何を実装するか」 |

---

## 2. GUIDE.md + TEMPLATE.md パターン

各サブディレクトリには必ず**GUIDE.md**（ルール定義）と**TEMPLATE.md**（コピー元）を配置する。

**GUIDE.md**: 目的・構成・命名規則・AIエージェント向けルール・品質チェックリストを定義。推奨構成:

```
1. 目的 → 2. ディレクトリ構成 → 3. 種別 → 4. 記法規約
→ 5. AIエージェント向けルール → 6. テンプレートの使用 → 7. 品質チェックリスト
→ 8. 参考資料 → 9. 更新履歴
```

**TEMPLATE.md**: 全セクションを含む汎用テンプレート。GUIDE.md内に**種別ごとのセクション取捨選択表**（必須/任意/不要）を設け、不要なセクションは削除して使用する。空セクションを残さない。

---

## 3. ライフサイクル管理

### resolved/ パターン

issues/ と plans/ では、`resolved/` サブディレクトリで状態を管理する。

```
docs/issues/
├── 012-current-bug.md        # 未解決（アクティブ）
└── resolved/
    └── 001-fixed-bug.md      # 解決済み
```

**ルール**:
- 解決/完了時に `resolved/` へ移動
- ファイル名はリネームしない
- 移動後の内容は編集しない（完了時点の状態を保持）
- 番号の欠番は埋めない

### archive/ パターン

歴史的に参照価値があるが現行でないドキュメントは `archive/` に移動する。

---

## 4. 命名規則

| 対象 | 規則 | 例 |
|------|------|-----|
| ガイド・テンプレート | `UPPERCASE.md` | `GUIDE.md`, `TEMPLATE.md` |
| ルートリファレンス | `UPPERCASE.md` | `COMMANDS.md`, `DEVELOPMENT.md` |
| モジュール設計書 | `lowercase.md` | `handlers.md`, `cache.md` |
| フロー設計書 | `{action}_flow.md` | `push_flow.md`, `init_flow.md` |
| ユースケース | `{command}.md` | `push.md`, `init.md` |
| Issues / 実装計画 | `{3桁番号}-{kebab-case}.md` | `001-block-id-lost.md` |

番号付きドキュメント: 3桁ゼロ埋め連番、既存最大+1で採番、欠番は埋めない。

---

## 5. メタデータコメント

番号付きドキュメントと設計書には冒頭にHTMLコメントでメタデータを記載する。

```markdown
<!--
種別: {カテゴリ固有の値}
優先度: 高 | 中 | 低
作成日: YYYY-MM-DD
-->
```

カテゴリ固有のフィールド:

| カテゴリ | 種別の値 | 追加フィールド |
|---------|---------|--------------|
| issues | `bug`, `design-gap`, `documentation`, `enhancement`, `refactoring` | `発見経路` |
| plans | `bugfix`, `missing_docs`, `enhancement`, `refactoring` | `担当`, `状態`（⏳/🏃/✅/⛔） |
| design | `modules`, `flows`, `usecase_mapping`, `decisions` | `対象`, `更新日`, `担当` |

---

## 6. 記法規約

### コード参照

ファイルパスと行番号で特定箇所を示す:

```markdown
`CacheManager::add_mapping()`（`crates/nob2md-core/src/cache/manager.rs:123`）
```

### コードスニペット

- **10行以内**: インライン記載（ソース位置をコメントで注記）
- **11行以上**: 参照リンクのみ + 要点を箇条書き

### 図表

- **Mermaid**: フローチャート、シーケンス図
- **テーブル**: API一覧、データ構造、エラーカタログ

### 相互参照

相対パスで参照する。`resolved/` 内はパスに含める。

```markdown
- [Issue #001](./resolved/001-block-id-lost.md)
- [設計書: handlers](../design/modules/handlers.md)
```

---

## 7. 各カテゴリの設計方針

### 7.1 design/ — 設計ドキュメント

実装済みコードの構造・設計パターンを文書化する（将来設計ではなく現状記録）。

| サブディレクトリ | 目的 |
|----------------|------|
| `modules/` | モジュール内部設計（公開API、依存関係、テストカバレッジ） |
| `flows/` | コマンド実行時の処理フロー（Mermaid図、エラーパス） |
| `decisions/` | 主要設計判断のADR（決定・理由・代替案・トレードオフ） |
| `usecase_mapping/` | ユースケースと実装の対応・カバレッジ |

詳細な構成・ADR形式は `design/GUIDE.md` と `design/TEMPLATE.md` を参照。

### 7.2 usecases/ — ユーザー向けドキュメント

**人間ユーザーとAIエージェントの両方**を対象とするデュアルオーディエンス設計。

| 観点 | 人間ユーザー | AIエージェント |
|-----|------------|--------------|
| フロー | 対話的（進捗バー、確認プロンプト） | 決定的（`--non-interactive`, `--format json`） |
| エラー対処 | メッセージの解釈 | 終了コード、検証ループ、自動修正 |
| 出力形式 | テキスト（色付き） | JSON（機械解析可能） |

詳細な構成は `usecases/GUIDE.md` と `usecases/TEMPLATE.md` を参照。

### 7.3 issues/ — 問題追跡（GitHub Issues に移行済み）

Issue管理は GitHub Issues に移行済み。`docs/issues/GUIDE.md` に旧ID→GitHub Issue の対応表を残している。新規Issueは GitHub Issues で作成すること。

### 7.4 plans/ — 実装計画

ギャップ分析、優先度付け、チーム作業計画を管理する。タスクID規約: `{ファイル番号}-{連番}`（例: `011-01`）。

タスクテーブル形式・スコープ表記は `plans/GUIDE.md` と `plans/TEMPLATE.md` を参照。

---

## 8. CLAUDE.md との連携

CLAUDE.md は**概要とナビゲーション**に徹し、詳細は docs/ へのリンクで段階的に開示する。

```markdown
## Handler
- 新Block type追加時は両方向の Handler を実装
- 詳細: [docs/design/modules/handlers.md](docs/design/modules/handlers.md)

## ドキュメントインデックス
| ドキュメント | 内容 |
|-------------|------|
| [docs/design/modules/](docs/design/modules/) | モジュール詳細設計 |
| [docs/usecases/](docs/usecases/) | ユースケース |
```

**参照の深さルール**:
- CLAUDE.md → docs/ のファイル: **1レベルまで**
- docs/ 内のファイル間参照は自由（同一 docs/ 内に閉じる）

---

## 9. AIエージェント向けの設計原則

### 調査の並列化

設計書作成では依存関係を考慮し、基盤モジュール → 変換モジュール → 統合モジュール → レビュー の順で並列調査する。

### 調査の深さ

必要最小限の深さで調査する。公開APIは関数シグネチャのみ、主要な実装はアルゴリズム概要、複雑なロジックのみ詳細分析。

### テストケースの活用

実装の動作仕様を理解するため、テストケース（正常系・異常系・境界値）を積極的に参照し、設計書内で引用する。

### フィードバックループ

ユースケースドキュメントでは、AIエージェント向けに「実行→検証→修正→再実行」の検証ループを必ず含める。

---

## 10. 品質チェックリスト

新しいドキュメントカテゴリを追加する際の確認事項:

- [ ] GUIDE.md が配置されている
- [ ] TEMPLATE.md が配置されている
- [ ] 種別ごとのセクション取捨選択表が GUIDE.md 内にある
- [ ] ライフサイクルが定義されている（resolved/ ルール等）
- [ ] ファイル命名規則が明確に定義されている
- [ ] メタデータコメントの形式が定義されている
- [ ] 相互参照が正しい相対パスを使用している
- [ ] CLAUDE.md のドキュメントインデックスに追加されている

---

## 11. 適用時の判断基準

全てのプロジェクトに全カテゴリが必要なわけではない。規模に応じて段階的に導入する。

```
docs/
├── COMMANDS.md              # ← 最小構成
├── DEVELOPMENT.md           # ← 最小構成
├── design/                  # ← 標準構成で追加（modules/ から開始）
├── usecases/                # ← 標準構成で追加
├── issues/                  # ← フル構成で追加
├── plans/     # ← フル構成で追加
└── archive/                 # ← フル構成で追加
```

| 条件 | 追加するカテゴリ |
|------|----------------|
| CLIツールである | `COMMANDS.md` |
| 複数モジュールがある | `design/modules/` |
| 処理フローが複雑 | `design/flows/` |
| アーキテクチャ判断を記録したい | `design/decisions/` |
| AIエージェントが使用する | `usecases/`（デュアルオーディエンス形式） |
| バグ追跡を管理する | GitHub Issues（`issues/` は移行ガイドのみ残す） |
| 複数人/エージェントで開発する | `plans/` |
