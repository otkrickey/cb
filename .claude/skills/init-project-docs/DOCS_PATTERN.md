# docs ディレクトリ設計パターン

## 全体構成

```
docs/
├── archive/                     # 歴史的ドキュメント（初期仕様書等）
├── design/                      # 設計ドキュメント
│   ├── GUIDE.md                 # 設計書作成ガイド
│   ├── TEMPLATE.md              # 設計書テンプレート
│   ├── decisions/               # アーキテクチャ決定記録（ADR）
│   ├── modules/                 # モジュール別設計書
│   ├── flows/                   # 処理フロー図（Mermaid）
│   └── usecase_mapping/         # ユースケースと実装の対応表
├── plans/                       # 実装計画
│   ├── GUIDE.md
│   ├── TEMPLATE.md
│   └── resolved/                # 完了済み計画
├── review/                      # コードレビューガイドライン
│   ├── GUIDE.md
│   ├── TEMPLATE.md
│   ├── FORMAT.md                # 共通出力形式
│   └── modules/                 # モジュール別チェックリスト
├── usecases/                    # ユーザー向け操作ドキュメント
│   ├── GUIDE.md
│   └── TEMPLATE.md
└── status/                      # 実装ステータス
    ├── GUIDE.md
    ├── implementation.md
    └── roadmap.md
```

## レイヤーの役割

| レイヤー | 問いに答える |
|---------|------------|
| **design/** | 「なぜこう作るか」「内部でどう動くか」 |
| **plans/** | 「次に何を実装するか」 |
| **review/** | 「品質基準を満たしているか」 |
| **usecases/** | 「どう使うか」 |
| **status/** | 「今どこまでできているか」 |
| **archive/** | 「最初の構想は何だったか」 |

## 設計原則

### 関心の分離

- 設計書 = 「あるべき姿」（実装ステータスを書かない）
- 実装計画 = 「やること」（タスク管理）
- ステータス = 「現在の状態」（進捗記録）

### GUIDE.md + TEMPLATE.md パターン

各カテゴリに必ず配置する:
- **GUIDE.md**: 目的・構成・命名規則・品質チェックリスト
- **TEMPLATE.md**: 全セクションを含む汎用テンプレート
- GUIDE.md内に**種別ごとのセクション取捨選択表**（必須/任意/不要）を設ける

### 命名規則

| 対象 | 規則 | 例 |
|------|------|-----|
| ガイド・テンプレート | `UPPERCASE.md` | `GUIDE.md`, `TEMPLATE.md` |
| モジュール設計書 | `lowercase.md` | `core.md`, `storage.md` |
| フロー設計書 | `{action}_flow.md` | `clipboard_flow.md` |
| ADR・実装計画 | `{3桁番号}-{kebab-case}.md` | `001-technology-stack.md` |
| ユースケース | `{operation}.md` | `clipboard.md` |

### メタデータコメント

番号付きドキュメントと設計書の冒頭にHTMLコメントで記載:

```markdown
<!--
種別: {カテゴリ固有の値}
対象: {対象名}
作成日: YYYY-MM-DD
更新日: YYYY-MM-DD
担当: {作成者}
-->
```

### ライフサイクル

- plans/: 完了時に `resolved/` へ移動。ファイル名はリネームしない
- archive/: 歴史的参照用。編集しない
