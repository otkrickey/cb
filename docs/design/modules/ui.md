<!--
種別: modules
対象: UI（SwiftUIビュー・ViewModel・ウィンドウ管理）
作成日: 2026-02-16
更新日: 2026-02-16
担当: AIエージェント
-->

# UI モジュール設計

## 概要

SwiftUIビュー、ViewModel、ウィンドウ管理、入力ハンドリングを含むUI層全体の設計。

**スコープ**:
- アプリエントリポイント（`CBApp`）、ライフサイクル（`AppDelegate`）
- メニューバー常駐（`MenuBarExtra`）+ 設定画面（`Settings Scene`）
- 履歴パネル表示（`HistoryWindowController` + `KeyablePanel`）
- ビュー（`HistoryPanel`, `ClipboardItemRow`, `SettingsView`, `ShortcutRecorderView`）
- ViewModel（`HistoryViewModel`, `SelectionState`）
- クリップボード監視（`ClipboardMonitor`）
- ペースト操作（`PasteService`）— 通常/プレーンテキスト
- グローバルショートカット（`ShortcutManager`）— カスタマイズ対応
- 暗号鍵管理（`KeychainManager`）

---

## 責務と境界

**責務**:
- ユーザーへの履歴表示・操作UI
- クリップボード変更のポーリング検知
- グローバルホットキーの登録・コールバック（カスタマイズ対応）
- 選択エントリのペースト実行（通常/プレーンテキスト）
- ドラッグ&ドロップによるデータ提供
- ユーザー設定の管理（UserDefaults / @AppStorage）
- アクセシビリティ権限の誘導

**境界**:
- データの永続化はRust FFI経由（直接DBアクセスしない）
- 検索はRust FFI経由のFTS5全文検索（デバウンス0.3秒）

---

## 公開API

### アプリ構造

`CBApp`（`@main`）→ `MenuBarExtra` + `Settings Scene` + `AppDelegate`

- `MenuBarExtra`: "Show History ⌥⌘V"、"Settings..."（⌘,）、"Quit"（⌘Q）
- `Settings Scene`: `SettingsView`を表示

`AppDelegate.applicationDidFinishLaunching()`:
1. `initStorage()`（同期） — Keychain暗号鍵取得 → マイグレーション → 暗号化DB初期化 → 起動時クリーンアップ
2. `Task { @MainActor in }` ブロックで以下を非同期実行:
   1. `ClipboardMonitor` 生成（`init()`内で`startMonitoring()`を自動実行）
   2. `HistoryWindowController` 生成
   3. `ShortcutManager` 生成 → `onTogglePanel`コールバック設定 → `start()`で明示的にホットキー登録
   4. `checkAccessibilityPermission()` — AX権限未許可時に誘導ダイアログ表示

`AppDelegate.initStorage()`:
1. Application Support ディレクトリ作成（`~/Library/Application Support/CB/`）
2. `KeychainManager.getOrCreateKey()` で暗号化キーを取得（なければ自動生成）
3. 既存DBがプレーンか判定（SQLiteヘッダ `"SQLite format 3"` チェック）→ プレーンなら`clipboard_plain.db`にリネーム
4. `clipboard_plain.db`が存在する場合、`migrate_database()` で暗号化DBへ変換 → 成功時にplainDB削除
5. `init_storage(dbPath, encryptionKey)` で暗号化Storage初期化
6. `cleanup_old_entries()` で保持期間（UserDefaults `retentionDays`、デフォルト7日）超過エントリを削除

`AppDelegate.checkAccessibilityPermission()`:
1. `AXIsProcessTrusted()` がtrueならスキップ
2. 1秒ディレイ後にNSAlertダイアログを表示（日本語）
3. 「システム設定を開く」選択時に`AXIsProcessTrustedWithOptions`でシステムダイアログを表示

### ClipboardMonitor（`ClipboardMonitor.swift`）

`@MainActor @ObservableObject`。0.5秒間隔の`Timer`で`NSPasteboard.general.changeCount`をポーリング。

| プロパティ/メソッド | 説明 |
|-------------------|------|
| `latestEntryTimestamp` | `@Published` — 最新保存時刻（UI更新トリガー） |
| `skipNextChange` | 自前ペースト時のセルフループ防止フラグ |
| `isChecking` | `private` — Timer多重実行防止ガード。前回のcheckClipboardが完了するまで新しいチェックをスキップ |
| `startMonitoring()` | タイマー開始（`isChecking`ガード付き） |
| `stopMonitoring()` | タイマー停止 |
| `checkClipboard()` | changeCount比較 → 型判定 → `Task.detached`でRust FFI保存（メインスレッド非ブロック） |

**コンテンツ型判定ロジック**:
- `NSPasteboard.string(forType: .string)` → テキスト取得
  - 前後空白をトリミング → `/`または`~`始まり（チルダ展開対応）、かつ単一行、かつファイルまたは親ディレクトリが`FileManager.fileExists`で存在確認 → `FilePath`
  - それ以外 → `PlainText`
- `.tiff`または`.png`データ → `Image`
- コンテンツハッシュで重複をスキップ

**監視開始**: `init()`内で`startMonitoring()`を自動呼び出し（AppDelegateでの明示呼び出し不要）

**解放**: `deinit`で`MainActor.assumeIsolated { timer?.invalidate() }`によりタイマーを安全に停止

**FFI保存のバックグラウンド実行**: `save_clipboard_entry` / `save_clipboard_image` は `Task.detached` でバックグラウンドスレッドから呼び出す。メインスレッドでのMutexロック取得によるUIフリーズを防止

### HistoryWindowController（`HistoryWindowController.swift`）

`@MainActor`。`KeyablePanel`（NSPanelサブクラス）の生成・表示・非表示を管理。`previousApp: NSRunningApplication?`でペースト先アプリを追跡。

| メソッド | 説明 |
|---------|------|
| `toggle()` | パネル非表示時は`show()`。表示中は`cycleTypeFilter()`でコンテンツタイプフィルタを切り替え（All → Text → Images → Files → All） |
| `show()` | `previousApp`に前面アプリを記憶 → `typeFilter`リセット → エントリ読み込み → パネル表示 |
| `hide()` | パネル非表示 |
| `selectAndPaste(asPlainText:)` | `touch_entry(id)`でコピー回数更新 → 選択エントリをペースト → `hide()` → `previousApp`をactivate → 200ms後に`simulatePaste()` |
| `handleKey(NSEvent)` | ↑↓: 選択移動、Return: ペースト、Shift+Return: プレーンテキストペースト、Esc: 閉じる |

**KeyablePanel**（`NSPanel`サブクラス）:
- `canBecomeKey = true` / `canBecomeMain = true`
- `sendEvent()`オーバーライドでナビゲーションキー（↑↓, Return, Esc）をインターセプト
- フローティング、ボーダーレス、透明背景、720x480

### ShortcutManager（`ShortcutManager.swift`）

Carbon Event Manager による⌥⌘V グローバルホットキー登録。UserDefaultsからカスタムキー設定を読み込み。

| メソッド | 説明 |
|---------|------|
| `start()` | UserDefaultsから`shortcutKeyCode`/`shortcutModifiers`を読み込み、`RegisterEventHotKey`でホットキー登録。デフォルトは⌥⌘V。Carbon EventHandlerは初回のみ`InstallEventHandler`でインストール（`eventHandlerInstalled`フラグで多重登録を防止） |
| `stop()` | `UnregisterEventHotKey`でホットキー解除（EventHandlerは解除しない） |
| `reregister()` | `stop()` → `start()`で再登録。`UserDefaults.didChangeNotification`監視で設定変更時に自動呼び出し |

コールバックで`onTogglePanel`クロージャを呼び出し。

### PasteService（`PasteService.swift`）

`@MainActor enum`（インスタンス化不可）。

| メソッド | 説明 |
|---------|------|
| `copyToClipboard(entry, imageData, monitor, asPlainText)` | NSPasteboardにコンテンツ設定。`asPlainText=true`時はテキストのみ`.string`型で設定。`monitor.skipNextChange = true`でセルフループ防止 |
| `simulatePaste()` | `AXIsProcessTrusted()`確認 → CGEventで⌘+Vキーストロークをシミュレート |

### HistoryViewModel（`HistoryViewModel.swift`）

`@MainActor @Observable`。Rust FFIからJSON経由でエントリを取得し、UI用に加工。

| プロパティ/メソッド | 説明 |
|-------------------|------|
| `entries` | 全エントリ配列 |
| `searchText` | 検索文字列（FTS5検索トリガー、デバウンス0.3秒） |
| `searchResults` | FTS5検索結果の一時保持配列 |
| `filteredEntries` | `typeFilter`適用後の表示用配列（`searchText`空時は`entries`、検索時は`searchResults`がベース） |
| `typeFilter` | `ContentTypeFilter` — コンテンツタイプフィルタ（`.all` / `.plainText` / `.image` / `.filePath`） |
| `imageCache` | `NSCache<NSNumber, NSImage>` — 画像キャッシュ（countLimit: 100、totalCostLimit: 50MB） |
| `targetAppName` | ペースト先アプリ名（ボトムバー表示用） |
| `hasMore` / `isLoadingMore` | ページネーション管理フラグ |
| `loadEntries()` | Rust FFI `get_recent_entries(50)` → `FFIResponse`ラッパーデコード → `updateFilteredEntries()`。エラー時はログ出力 |
| `loadMoreEntries()` | Rust FFI `get_entries_before(cursor, 50)` → `FFIResponse`ラッパーデコード → カーソルベースページネーション → `updateFilteredEntries()` |
| `performSearch()` | デバウンス0.3秒 → Rust FFI `search_entries(query, 50)` → `FFIResponse`ラッパーデコード → `searchResults`更新 → `updateFilteredEntries()` |
| `cycleTypeFilter()` | `typeFilter`を次の値に切り替え（All → Text → Images → Files → All） → `updateFilteredEntries()` |
| `loadImage(for:)` | キャッシュヒット時は同期返却。ミス時は`nil`返却 + `Task { @MainActor }`で非同期ロード → キャッシュ登録 → UI再描画。`loadingImageIds`で重複ロード防止 |
| `loadImageData(for:)` | Rust FFI `get_entry_image()` → `Data`として返却（ペースト用） |
| `deleteEntry(_:)` | Rust FFI `delete_entry()` → 成功時のみローカル配列から削除 → `updateFilteredEntries()`。失敗時は変更なし |
| `shouldShowDateHeader(at:)` | 日付グループヘッダ表示判定 |
| `dateHeader(for:)` | "Today" / "Yesterday" / フォーマット済み日付 |

### SelectionState（`SelectionState.swift`）

`@MainActor @Observable`。キーボードナビゲーション用の選択インデックス管理。

| プロパティ/メソッド | 説明 |
|-------------------|------|
| `selectedIndex` | 現在の選択行インデックス（初期値: 0） |
| `entryCount` | 表示中のエントリ総数（境界チェック用） |
| `moveUp()` | `selectedIndex > 0` の場合にデクリメント |
| `moveDown()` | `selectedIndex < entryCount - 1` の場合にインクリメント |

### SettingsView（`Views/SettingsView.swift`）

macOS Settings Sceneで表示される設定画面。`@AppStorage`でUserDefaultsと連携。

| セクション | 内容 |
|-----------|------|
| データ保持 | 保持期間 Picker（1〜7日間、デフォルト7日） |
| ショートカット | `ShortcutRecorderView`（キー設定）、ペースト操作説明（⏎ / ⇧⏎） |
| 情報 | アプリバージョン（`CFBundleShortVersionString`） |

### ShortcutRecorderView（`Views/ShortcutRecorderView.swift`）

グローバルショートカットを変更するためのキー入力キャプチャUI。

- ボタンに現在のショートカット文字列を表示（例: `⌥⌘V`）。recording中は「Done」を表示
- ボタンクリックで入力待ち状態 → `NSEvent.addLocalMonitorForEvents(.keyDown)`でキャプチャ
- 修飾キー必須のバリデーション（修飾キーなしは拒否）
- 「Reset」でデフォルト（⌥⌘V）にリセット
- `@AppStorage("shortcutKeyCode")` / `@AppStorage("shortcutModifiers")` で永続化

### HistoryPanel（`Views/HistoryPanel.swift`）

二ペインレイアウトの履歴パネルビュー。`GlassEffectContainer` + `.glassEffect(.regular, in: .rect(cornerRadius: 16))` で720x480。

**フォーカス管理**:
- `@FocusState isSearchFocused`: サーチフィールドのフォーカス状態
- `onAppear`でサーチにフォーカス設定

**詳細プレビュー**:
- テキスト: `SelectableTextView`（`NSViewRepresentable`）— 内部で`NSTextView`（`isEditable=false`, `isSelectable=true`）を使用し、テキスト選択 → ⌘+Cコピー対応
- 画像: `Image(nsImage:)` + `.aspectRatio(contentMode: .fit)`。非同期ロード中は`ProgressView` + "Loading image..."を表示
- Information セクション: ソースアプリ / コンテンツタイプ / 文字数 / ワード数 / 画像サイズ / コピー回数（Times copied） / 最終コピー日時（Last copied） / 初回コピー日時（First copied、`copy_count > 1`時のみ）

### ClipboardItemRow（`Views/ClipboardItemRow.swift`）

履歴リストの各行。SF Symbolアイコン + 1行プレビューテキスト + 日付グループヘッダ。

**ドラッグ&ドロップ**: `.onDrag()` で`NSItemProvider`を提供:
- テキストエントリ → `NSString`
- 画像エントリ → `NSImage`

### KeychainManager（`KeychainManager.swift`）

`enum`（インスタンス化不可）。macOS Keychainを使ったDB暗号化キーの管理。

| メソッド | 説明 |
|---------|------|
| `getOrCreateKey()` | Keychainからキーを取得。なければCryptoKitで256ビット対称鍵を生成・保存 |
| `getKey()` | Keychainから既存キーを取得（`SecItemCopyMatching`） |
| `saveKey(_:)` | 新規キーをKeychainに保存（`SecItemAdd`） |

**Keychain設定**:
- Service: `com.otkrickey.cb.db-encryption`
- Account: `clipboard-db-key`
- `kSecAttrAccessibleAfterFirstUnlock`: 初回ログイン後バックグラウンドでもアクセス可能
- `kSecAttrSynchronizable: false`: iCloud同期を無効化（端末限定）

---

## エラーハンドリング

| エラー種別 | 発生条件 | 対処 |
|-----------|---------|------|
| Rust FFI失敗 | DB接続エラー等 | `false`返却 → サイレント失敗。`deleteEntry`は削除をスキップ |
| FFIエラーレスポンス | DB接続エラー、Storage未初期化等 | `FFIResponse.error`をログ出力（`os.Logger` category: `HistoryViewModel`）。UIは現状維持 |
| JSONデコード失敗 | FFIからの不正JSON | `FFIResponse`デコード失敗時は空配列にフォールバック |
| アクセシビリティ未許可 | `AXIsProcessTrusted() == false` | 起動時にNSAlert誘導ダイアログ表示。`simulatePaste()`はスキップ |
| Keychain取得失敗 | Keychainアクセス不可 | `initStorage()`が中断、DB未初期化 |
| DBマイグレーション失敗 | 平文→暗号化変換エラー | `logger.error`でログ出力、旧DBを保持 |

---

## ロギング

macOS統合ログシステム（`os.Logger`）を使用。subsystem `com.otkrickey.cb` で統一し、ファイルごとにcategoryを分離。

**確認方法**: Console.appまたはターミナルで `log stream --predicate 'subsystem == "com.otkrickey.cb"'` を実行。

| ファイル | category | 主な出力内容 |
|---------|----------|------------|
| `AppDelegate` | `AppDelegate` | 起動シーケンス、DB初期化、マイグレーション、クリーンアップ |
| `HistoryWindowController` | `HistoryWindowController` | パネル表示/非表示、タイプフィルタ切り替え、ペースト先アプリ |
| `ShortcutManager` | `ShortcutManager` | ホットキー登録/再登録、キー押下検知 |
| `PasteService` | `PasteService` | AX権限警告、CGEvent生成失敗 |
| `KeychainManager` | `KeychainManager` | 暗号化キー生成・保存 |
| `HistoryViewModel` | `HistoryViewModel` | FFIエラーレスポンスのログ出力 |
| `ClipboardMonitor` | `ClipboardMonitor` | FFI保存失敗のエラーログ出力 |

**ログレベル**:
- `notice`: 正常動作の記録（起動完了、ホットキー登録成功、パネル操作等）
- `warning`: 機能制限を伴う状態（AX権限未許可、previousApp不明）
- `error`: 操作失敗（DB初期化失敗、マイグレーション失敗、Keychainエラー、CGEvent生成失敗、FFI保存失敗）

**対象外**: `SelectionState`、Views（`HistoryPanel`、`ClipboardItemRow`、`SettingsView`、`ShortcutRecorderView`）にはLoggerを使用していない。

---

## 関連ドキュメント

- [UIデザインシステム ADR](../decisions/002-ui-design-system.md)
- [データベース暗号化 ADR](../decisions/003-database-encryption.md)
- [cb-core モジュール設計](./cb-core.md)
- [ペーストフロー](../flows/paste_flow.md)
