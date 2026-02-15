# CLAUDE.md 作成ガイド

Claude Code向けの`CLAUDE.md`を効果的に書くための記法・構成パターン集。
[agent-skills-best-practices.md](agent-skills-best-practices.md)から抽出した知見を、CLAUDE.md作成に応用可能な形でまとめる。

---

## 1. 簡潔さが最重要

CLAUDE.mdの内容はコンテキストウィンドウを消費する。全ての記述について以下を自問する：

- Claudeが既に知っている情報ではないか？
- そのトークンコストに見合う価値があるか？

**良い例**（必要な情報のみ）：
```markdown
## ビルド

cargo build --workspace
cargo test --workspace
```

**悪い例**（冗長）：
```markdown
## ビルド

このプロジェクトはRustで書かれています。RustはMozillaが開発した
システムプログラミング言語で、メモリ安全性を保証します。
ビルドにはCargoというビルドシステムを使います。Cargoは
Rustの公式パッケージマネージャーで...
```

Claudeは汎用知識（言語仕様、一般的なツールの使い方等）を既に持っている。プロジェクト固有の情報だけを書く。

## 2. 自由度の設計

記述の具体性を、タスクの脆弱性に応じて使い分ける。

### 高い自由度（テキストベースの指示）

複数のアプローチが有効で、コンテキスト次第で判断が変わる場合：

```markdown
## コードレビュー

1. コード構造と組織を分析する
2. 潜在的なバグやエッジケースをチェックする
3. プロジェクト規約への準拠を確認する
```

### 中程度の自由度（パターン提示）

推奨パターンはあるが、変動を許容する場合：

````markdown
## Handler実装

新しいBlock typeを追加する際のテンプレート：

```rust
impl MdToNotionHandler for NewBlockHandler {
    fn can_handle(&self, node: &Node) -> bool { /* 判定ロジック */ }
    fn convert(&self, node: &Node) -> Result<NotionBlock> { /* 変換ロジック */ }
}
```

必要に応じてヘルパー関数を追加してよい。
````

### 低い自由度（厳密な手順）

一貫性が重要で、特定の手順に従う必要がある場合：

````markdown
## データベースマイグレーション

このコマンドを正確に実行する：

```bash
cargo run -- migrate --verify --backup
```

フラグを変更・追加しないこと。
````

**判断基準**：失敗時のリスクが高い操作ほど自由度を下げる。

## 3. 段階的開示パターン

CLAUDE.mdに全てを書かず、詳細は別ファイルに分離する。

```markdown
## アーキテクチャ

### Handler System
- Handler Registryが各Block typeの変換を管理
- 詳細: [docs/design/modules/handlers.md](docs/design/modules/handlers.md)

### キャッシュ管理
- ファイルパス↔Page IDのマッピングを管理
- 詳細: [docs/design/modules/cache.md](docs/design/modules/cache.md)
```

### ルール

- **CLAUDE.mdは概要とナビゲーション**に留める
- **参照は1レベルまで**（CLAUDE.md → 参照ファイル。参照ファイル → さらに別の参照、は避ける）
- 長い参照ファイルには**目次**を付ける

```markdown
# APIリファレンス

## 目次
- 認証とセットアップ
- コアメソッド（作成、読み取り、更新、削除）
- エラー処理パターン

## 認証とセットアップ
...
```

## 4. テンプレートと例の提示

### テンプレートパターン

出力形式に一貫性が必要な場合、テンプレートを提供する：

````markdown
## コミットメッセージ

形式：`type(scope): 簡潔な説明`

```
feat(handler): テーブルBlock変換を実装

- table_width, has_column_headerフィールドに対応
- ネストされたtable_rowを再帰的に処理
```
````

### 入出力例パターン

期待する品質を例で示す：

````markdown
## 例

**入力**: heading_2 Block (`{ type: "heading_2", text: "Section" }`)
**出力**: mdast Heading node (`{ type: "heading", depth: 2, children: [...] }`)
````

例は抽象的な説明より効果的にClaudeの出力品質を制御できる。

## 5. ワークフローとチェックリスト

複数ステップの操作にはチェックリストを活用する：

````markdown
## 新しいHandlerの追加手順

```
- [ ] MdToNotionHandler traitを実装
- [ ] NotionToMdHandler traitを実装
- [ ] HandlerRegistryに登録
- [ ] ユニットテスト作成
- [ ] スナップショットテスト作成
- [ ] ラウンドトリップテスト確認
```
````

チェックリストはClaudeが進捗を追跡し、ステップの飛ばしを防ぐのに有効。

## 6. 条件分岐の記述

状況に応じて異なる手順を取る場合、分岐を明示する：

```markdown
## エラーハンドリング

変更タイプに応じて対応を選択する：

**未対応のBlock typeの場合** → UnsupportedBehavior設定に従う
**API制限超過の場合** → truncateまたはエラーを返す
**ネットワークエラーの場合** → 指数バックオフでリトライ
```

## 7. 一貫した用語

CLAUDE.md全体で用語を統一する。混在は混乱を招く。

| 統一する | 混在を避ける |
|---------|------------|
| Block | Block, ブロック, block, element |
| Handler | Handler, ハンドラー, converter, 変換器 |
| mdast | mdast, AST, 構文木, syntax tree |

## 8. 避けるべきアンチパターン

### 時間依存の情報
```markdown
# 悪い例
2025年8月前はv1 APIを使用してください。

# 良い例
v2 APIを使用する。レガシーv1 APIの情報は docs/legacy/ を参照。
```

### 選択肢の提示しすぎ
```markdown
# 悪い例
pdfplumberまたはPyMuPDFまたはpdf2imageで処理できます。

# 良い例
テキスト抽出にはpdfplumberを使用する。
OCRが必要な場合のみpdf2image + pytesseractを使用する。
```

### 深いネスト参照
```markdown
# 悪い例（3段階のネスト）
CLAUDE.md → advanced.md → details.md → 実際の情報

# 良い例（1段階）
CLAUDE.md → 各参照ファイル（直接リンク）
```

## 9. CLAUDE.md構成テンプレート

推奨する基本構成：

```markdown
# CLAUDE.md

[プロジェクトの1行説明]

## プロジェクト概要
[Claudeが知らないプロジェクト固有の情報のみ]

## プロジェクト構造
[ディレクトリ構成とその役割]

## ビルド・テスト
[開発に必要なコマンド]

## アーキテクチャ
[核心的な設計判断と重要な概念]
[詳細は別ファイルへのリンク]

## 実装ルール
[守るべき規約・制約]

## 設定ファイル
[設定の構造と意味]

## ドキュメントインデックス
[関連ドキュメントへのリンク集]
```

## 10. チェックリスト：CLAUDE.mdレビュー

公開前に確認：

- [ ] Claudeが既に知っている情報を省略しているか
- [ ] プロジェクト固有の情報のみ記載しているか
- [ ] 500行以下に収まっているか（超える場合は別ファイルに分離）
- [ ] 参照リンクは1レベルまでか
- [ ] 用語が統一されているか
- [ ] 時間依存の情報を含んでいないか
- [ ] 高リスク操作には具体的な手順を示しているか
- [ ] 低リスク操作にはClaudeの裁量を許容しているか
- [ ] テンプレートや例で期待する品質を示しているか
