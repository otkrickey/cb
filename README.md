# CB

macOS向け高機能クリップボードマネージャー

## 特徴

- **クリップボード履歴の自動保存** — コピーした内容を自動的に記録・管理
- **Liquid Glass デザイン** — macOS 26（Tahoe）のデザインシステムを全面採用
- **グローバルショートカット** — ⌥⌘V で即座に履歴を呼び出し（カスタマイズ対応）
- **FTS5 全文検索** — 過去のコピー内容を高速に検索
- **SQLCipher AES-256 暗号化** — データベースをページレベルで暗号化、鍵は macOS Keychain で管理
- **マルチフォーマット対応** — テキスト・画像・ファイルパスなど各種データタイプをサポート
- **ドラッグ＆ドロップ** — 履歴アイテムをドラッグして他のアプリへ直接貼り付け
- **自動クリーンアップ** — 保持期間の設定により古いエントリを自動削除

## 必要環境

- macOS 26.2+
- Xcode 26.2+
- Rust（stable）
- [XcodeGen](https://github.com/yonaskolb/XcodeGen)

## インストール

### Homebrew

```bash
brew install --cask otkrickey/tap/cb
```

### GitHub Releases

[Releases](https://github.com/otkrickey/cb/releases) から最新の zip をダウンロードし、`CB.app` を `/Applications` に配置。

> 初回起動時に Gatekeeper でブロックされる場合: `xattr -cr /Applications/CB.app`

## ビルド

```bash
# Rust ライブラリのビルド
cargo build --workspace

# Xcode プロジェクトの生成
xcodegen generate

# アプリのビルド（pre-build スクリプトが cargo build を自動実行）
xcodebuild -project CB.xcodeproj -scheme CB build
```

## テスト

```bash
cargo test --workspace
```

## アーキテクチャ

UI を Swift（SwiftUI + AppKit）、ロジック・データ層を Rust で構築するハイブリッドアーキテクチャ。Swift と Rust の間は [swift-bridge](https://github.com/chinedufn/swift-bridge) による FFI で接続し、複雑な型は JSON 文字列、画像データは `&[u8]` で受け渡します。

```
┌─────────────────────────┐
│   Swift（UI + 監視）     │
│  SwiftUI / AppKit       │
│  ClipboardMonitor       │
│  PasteService           │
└──────────┬──────────────┘
           │ swift-bridge FFI
┌──────────▼──────────────┐
│   Rust（ロジック + データ）│
│  models / storage       │
│  SQLite (SQLCipher)     │
│  FTS5 全文検索           │
└─────────────────────────┘
```

## 技術スタック

| 層 | 技術 |
|---|---|
| UI | SwiftUI + AppKit（Liquid Glass） |
| ロジック | Rust |
| データベース | SQLite（rusqlite bundled-sqlcipher、AES-256 暗号化） |
| FFI | swift-bridge |
| ビルド | XcodeGen + Cargo |
| ホットキー | Carbon Event Manager |
| 暗号鍵管理 | macOS Keychain（CryptoKit） |
| 検索 | FTS5 全文検索 |

## ライセンス

[MIT License](LICENSE)
