<!--
種別: decisions
対象: データベース暗号化
作成日: 2026-02-16
更新日: 2026-02-16
担当: AIエージェント
-->

# データベース暗号化

## 概要

cbアプリのクリップボード履歴データベース（SQLite）を暗号化し、DBファイルの持ち出しによる情報漏洩を防止する。SQLCipherによるAES-256ページレベル暗号化を採用し、暗号鍵はmacOS Keychainで安全に管理する。

---

## 設計判断

### 判断1: 暗号化方式 — SQLCipher（bundled-sqlcipher）

**問題**: クリップボード履歴DBの暗号化をどの方式で実現するか

**選択肢**:
1. SQLCipher（rusqliteの`bundled-sqlcipher`フィーチャー）
2. アプリケーションレベル暗号化（`aes-gcm` crate等で各フィールドを個別暗号化）
3. FileVault依存（OS標準のディスク暗号化に委任）
4. 暗号化なし

**決定**: SQLCipher（`bundled-sqlcipher`）

**理由**:
- DB全体をAES-256でページレベル暗号化。テーブル名・カラム名・メタデータすべてが保護される
- `PRAGMA key` の1行追加のみで透過的に動作し、既存のSQLクエリ変更が不要
- 将来のFTS5全文検索との互換性が確保されている
- `sqlcipher_export` による平文→暗号化のマイグレーション手段が公式提供されている

**トレードオフ**:
- **利点**: 透過的暗号化、メタデータ保護、FTS5互換、公式マイグレーションツール
- **欠点**: 5-10%のパフォーマンスオーバーヘッド、バイナリサイズ約2MB増加

---

### 判断2: 暗号鍵管理 — macOS Keychain

**問題**: 暗号鍵をどこに保存し、どのように管理するか

**選択肢**:
1. macOS Keychain
2. ファイルベース（鍵ファイルをディスクに保存）
3. 環境変数
4. ユーザー入力（毎回パスワード入力）

**決定**: macOS Keychain

**理由**:
- OSレベルのセキュアストレージにより鍵が保護される
- アプリ起動時に自動取得でき、ユーザー操作が不要
- `kSecAttrSynchronizable: false` でiCloud同期を無効化し、端末限定で管理
- `kSecAttrAccessibleAfterFirstUnlock` により初回ログイン後はバックグラウンドでもアクセス可能

**トレードオフ**:
- **利点**: OSレベル保護、ユーザー操作不要、端末限定管理
- **欠点**: Keychain破損時に鍵喪失リスク（＝DB読み取り不可）、macOS依存

---

### 判断3: マイグレーション戦略 — sqlcipher_export

**問題**: 既存の平文DBをどのように暗号化DBへ変換するか

**決定**: SQLCipher公式の `sqlcipher_export` 関数で平文→暗号化変換

**理由**:
- SQLCipher公式推奨のマイグレーション手法
- 全テーブル・インデックスを一括変換
- `ATTACH DATABASE ... KEY ...` → `sqlcipher_export` → `DETACH` の3ステップで完了

**実装フロー**:
1. 既存の `clipboard.db` を `clipboard_plain.db` にリネーム
2. `sqlcipher_export` で `clipboard_plain.db` → 新 `clipboard.db`（暗号化）へ変換
3. 変換成功後、`clipboard_plain.db` を削除

---

## 関連ドキュメント

- [技術スタック選定 ADR](./001-technology-stack.md)
- [cb-core モジュール設計](../modules/cb-core.md)
- [UI モジュール設計](../modules/ui.md)（KeychainManager, AppDelegate.initStorage）
