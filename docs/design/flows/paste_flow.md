<!--
種別: flows
対象: ペースト操作フロー
作成日: 2026-02-16
更新日: 2026-02-16
担当: AIエージェント
-->

# ペースト操作フロー

## 概要

ユーザーがグローバルショートカットで履歴パネルを開き、エントリを選択してアクティブアプリにペーストするまでのフロー。

---

## 処理フロー

```mermaid
sequenceDiagram
    participant User
    participant ShortcutMgr as ShortcutManager
    participant WinCtrl as HistoryWindowController
    participant VM as HistoryViewModel
    participant Rust as cb-core (Rust FFI)
    participant Panel as HistoryPanel
    participant Paste as PasteService
    participant App as 前面アプリ

    User->>ShortcutMgr: ⌥⌘V 押下
    ShortcutMgr->>WinCtrl: onTogglePanel()
    WinCtrl->>WinCtrl: previousApp = frontmostApp
    WinCtrl->>VM: loadEntries()
    VM->>Rust: get_recent_entries(50)
    Note over Rust: SQLCipher暗号化DB<br/>からSELECT
    Rust-->>VM: JSON文字列
    VM->>VM: JSONDecodeしてentries更新
    WinCtrl->>Panel: パネル表示 (makeKeyAndOrderFront)
    Panel->>User: 履歴一覧表示

    User->>Panel: ↑↓キーで選択
    Panel->>WinCtrl: handleKey(event)
    WinCtrl->>WinCtrl: selectionState.moveUp/Down()

    User->>Panel: Return押下
    Panel->>WinCtrl: handleKey(.return)
    WinCtrl->>WinCtrl: selectAndPaste()
    WinCtrl->>Rust: touch_entry(id)
    Note over Rust: copy_count++ &<br/>created_at更新

    alt テキストエントリ
        WinCtrl->>Paste: copyToClipboard(entry)
        Paste->>Paste: monitor.skipNextChange = true
        Paste->>Paste: NSPasteboard.setString()
    else 画像エントリ
        WinCtrl->>VM: loadImageData(for: id)
        VM->>Rust: get_entry_image(id)
        Rust-->>VM: Vec<u8>
        WinCtrl->>Paste: copyToClipboard(entry, imageData)
        Paste->>Paste: NSPasteboard.writeObjects([NSImage])
    end

    WinCtrl->>Panel: hide()
    WinCtrl->>App: previousApp.activate()
    Note over WinCtrl,App: 0.2秒ディレイ
    WinCtrl->>Paste: simulatePaste()
    Paste->>Paste: AXIsProcessTrusted() 確認
    Paste->>App: CGEvent(⌘+V) 送信
```

## 各ステップの詳細

### 1. ホットキーコールバック

Carbon Event Managerが⌥⌘Vを検知 → グローバル変数`_shortcutManagerInstance`経由で`handleHotKey()` → `onTogglePanel`クロージャ呼び出し。

### 2. パネル表示

`HistoryWindowController.show()`:
1. `NSWorkspace.shared.frontmostApplication` を `previousApp` に記憶
2. `viewModel.loadEntries()` で最新50件をRust FFI経由で取得
3. 選択インデックスを0にリセット
4. パネルを画面中央やや上（垂直10%オフセット）に配置
5. `NSApp.activate(ignoringOtherApps: true)` でアプリをアクティブ化

### 3. キーボードナビゲーション

`KeyablePanel.sendEvent()`でキーコード126(↑), 125(↓), 36(Return), 53(Esc)をインターセプト。SwiftUIのTextFieldに到達する前に処理する。

### 4. ペースト実行

1. `PasteService.copyToClipboard()` — NSPasteboardにコンテンツ設定
2. `monitor.skipNextChange = true` — セルフループ防止
3. パネル非表示
4. `previousApp.activate()` — 元のアプリをアクティブ化
5. **0.2秒ディレイ** — アプリのアクティベーション完了を待つ
6. `PasteService.simulatePaste()` — CGEventで⌘+Vを送信

### 5. アクセシビリティ要件

`simulatePaste()`は`AXIsProcessTrusted()`がtrueの場合のみ動作。未許可の場合はサイレントスキップ。

---

## 関連ドキュメント

- [UI モジュール設計](../modules/ui.md)
- [クリップボード監視フロー](./clipboard_flow.md)
