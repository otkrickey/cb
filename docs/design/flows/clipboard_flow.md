<!--
種別: flows
対象: クリップボード監視・保存フロー
作成日: 2026-02-16
更新日: 2026-02-16
担当: AIエージェント
-->

# クリップボード監視・保存フロー

## 概要

ユーザーがテキストや画像をコピーしてから、SQLiteに保存されUIが更新されるまでのエンドツーエンドフロー。

---

## 処理フロー

```mermaid
flowchart TD
    A[ユーザーがコピー操作] --> B[NSPasteboard.changeCount 変化]
    B --> C{ClipboardMonitor<br/>0.5秒ポーリング}
    C -->|isChecking == true| C
    C -->|isChecking == false| C2[isChecking = true]
    C2 -->|changeCount 変化なし| C3[isChecking = false]
    C3 --> C
    C2 -->|changeCount 変化あり| D{skipNextChange?}
    D -->|true| E[フラグリセット<br/>スキップ]
    E --> C3
    D -->|false| F[コンテンツ取得]
    F --> G{コンテンツハッシュ<br/>前回と同一?}
    G -->|同一| C3
    G -->|異なる| H{コンテンツ種別判定}
    H -->|string取得成功| I{パス判定<br/>トリミング+単一行+正規表現}
    I -->|パスパターンにマッチ| J[FilePath]
    I -->|それ以外| K[PlainText]
    H -->|tiff/png取得成功| L[Image]
    H -->|いずれも取得失敗| C3
    J --> M[Task.detached:<br/>Rust FFI save_clipboard_entry]
    K --> M
    L --> N[Task.detached:<br/>Rust FFI save_clipboard_image]
    M --> O[SQLite INSERT<br/>バックグラウンド実行]
    N --> O
    O --> P[latestEntryTimestamp 更新]
    P --> Q[HistoryPanel<br/>onChange で再描画]
```

## 各ステップの詳細

### 1. ポーリング（ClipboardMonitor）

`Timer.scheduledTimer(withTimeInterval: 0.5)`で`NSPasteboard.general.changeCount`を前回値と比較。

### 2. セルフループ防止

`PasteService.copyToClipboard()`実行時に`monitor.skipNextChange = true`を設定。次のchangeCount変化を1回だけスキップし、自分自身のペーストを履歴に再保存しない。

### 3. 重複検出

コンテンツの`hashValue`を前回と比較。同一コンテンツの連続コピーをスキップ。

### 4. コンテンツ種別判定

| 優先度 | 条件 | 種別 |
|--------|------|------|
| 1 | `pasteboard.string(forType: .string)` 取得成功 | PlainText / FilePath |
| 2 | `pasteboard.data(forType: .tiff)` 取得成功 | Image |
| 3 | `pasteboard.data(forType: .png)` 取得成功 | Image |

テキストが取得できる場合はテキストを優先（画像を含むコピーでもテキスト表現がある場合がある）。

### 5. FFI呼び出し → SQLite保存

`Task.detached` でバックグラウンドスレッドからRust FFI関数を呼び出し、StorageシングルトンのMutexをロックしてSQLCipher暗号化SQLite INSERTを実行。メインスレッドをブロックしない設計。`created_at`は`SystemTime::now()`のUnixタイムスタンプ。DBは`init_storage(dbPath, encryptionKey)`で暗号化キー付きで初期化済み。

---

## 関連ドキュメント

- [cb-core モジュール設計](../modules/cb-core.md)
- [UI モジュール設計](../modules/ui.md)
- [ペーストフロー](./paste_flow.md)
