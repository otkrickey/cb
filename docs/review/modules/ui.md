# UI レビューガイドライン

## 対象

`CB/Sources/` 配下の変更（CBApp.swift, AppDelegate.swift, ClipboardMonitor.swift, HistoryWindowController.swift, PasteService.swift, ShortcutManager.swift, KeychainManager.swift, Views/, ViewModels/）

## 評価項目

1. **MainActorの一貫性** — UI操作を行うクラス・メソッドに `@MainActor` が付与されているか。バックグラウンドスレッドからのUI更新がないか
2. **クリップボード監視の正確性** — `ClipboardMonitor` の `changeCount` ポーリングとコンテンツハッシュによる重複スキップが正しく動作するか。`skipNextChange` によるセルフループ防止が漏れなく設定されるか
3. **ペースト操作の安全性** — `PasteService.simulatePaste()` で `AXIsProcessTrusted()` を確認しているか。`copyToClipboard` の `asPlainText` 分岐が正しいか。CGEventの生成・ポストが適切か
4. **グローバルショートカット** — `ShortcutManager` のCarbon Event Manager登録・解除が正しいか。`UserDefaults.didChangeNotification` による動的再登録でリソースリーク（旧ホットキー未解除）がないか
5. **Keychain管理** — `KeychainManager` の `kSecAttrSynchronizable: false`（iCloud同期無効）と `kSecAttrAccessibleAfterFirstUnlock` が設定されているか。キー生成にCryptoKit 256ビット対称鍵を使用しているか
6. **DB初期化フロー** — `AppDelegate.initStorage()` のステップ（ディレクトリ作成→Keychainキー取得→プレーンDB判定→マイグレーション→暗号化DB初期化→クリーンアップ）が設計書通りの順序で実行されるか
7. **ウィンドウ管理** — `KeyablePanel` の `sendEvent()` オーバーライドでキーイベント（↑↓, Return, Shift+Return, Esc）が正しくインターセプトされるか。パネルのフローティング・フォーカス管理が適切か
8. **画像キャッシュ** — `NSCache` の `countLimit`（100）と `totalCostLimit`（50MB）が設定されているか。キャッシュミス時のRust FFI呼び出しと結果のキャッシュ登録が正しいか
9. **ドラッグ&ドロップ** — `ClipboardItemRow` の `.onDrag()` で `NSItemProvider` がテキスト/画像に応じて正しい型で提供されるか
10. **設計書との整合** — `docs/design/modules/ui.md` の公開API・ライフサイクル・エラーハンドリング定義と実装が一致するか
