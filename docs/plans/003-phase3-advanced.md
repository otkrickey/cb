# Phase 3: 高度な機能

<!--
種別: enhancement
優先度: 中
作成日: 2026-02-16
担当: メインセッション
状態: ✅ 完了
-->

## 概要

Phase 2（生産性向上機能）で実装したクリップボードマネージャーに、キーボードショートカットのカスタマイズ・パフォーマンス最適化・CI/CDパイプラインを追加する。ユーザーの個別要件に応える操作性の向上、大量履歴でのスムーズな動作、そしてGitHub Releaseによる配布基盤を整備し、プロダクトとしての完成度を高める。

**背景**:
- グローバルショートカットが⌥⌘Vに固定されており、他のアプリとの競合やユーザーの好みに対応できない
- 履歴エントリの取得が50件固定（`get_recent_entries(50)`）で、古い履歴にアクセスできない
- 画像キャッシュが単純な`Dictionary`でメモリ制限がなく、大量画像エントリでメモリが肥大化する
- ビルド・テスト・リリースが手動で、配布の仕組みがない

**目的**:
- ユーザーが任意のグローバルショートカットを設定できるようにする
- ページネーションと画像キャッシュ最適化で大量履歴でも快適に動作させる
- GitHub Actionsでテスト自動化とリリースビルドの自動化を実現する

---

## 現状分析

### 不足している要素

- `ShortcutManager`のキーコード・修飾キーがハードコードされている（`kVK_ANSI_V` + `cmdKey | optionKey`）
- Rust側の`get_recent_entries`にオフセットパラメータがなく、ページネーション不可
- Swift側の画像キャッシュ（`Dictionary<Int64, NSImage>`）にLRU制限がない
- GitHub Actionsワークフローが未定義
- `project.yml`にコード署名・バージョン管理が未設定

### 影響範囲

| 影響対象 | 現状の問題 | 優先度 |
|---------|----------|--------|
| `ShortcutManager.swift` | キーがハードコード、動的変更不可 | 高 |
| `storage.rs` / `lib.rs` | ページネーション未対応 | 高 |
| `HistoryViewModel.swift` | 50件固定取得、画像キャッシュ無制限 | 高 |
| `HistoryPanel.swift` | 無限スクロール未対応 | 中 |
| `project.yml` | バージョン管理・コード署名なし | 中 |
| `.github/workflows/` | CI/CDパイプラインなし | 中 |

---

## 実装スコープ

### 対応範囲 ✅

- [x] キーボードショートカットのカスタマイズ（設定UI + 動的登録変更）
- [x] ページネーション（カーソルベース + Swift側無限スクロール）
- [x] 画像キャッシュ最適化（NSCache導入 + 100枚/50MB制限）
- [x] CI/CDパイプライン（GitHub Actions: テスト・ビルド・リリース）
- [x] バージョン管理・コード署名・エンタイトルメントの設定

### 対応外 ❌

- カスタムフォーマットペースト（理由: 複雑すぎるためスコープ外）
- iCloud同期（理由: 実装コストが高くニーズが限定的）
- スニペット管理（理由: 複雑すぎるためスコープ外）
- Notarization自動化（理由: Apple Developer Program加入が前提。手動対応で可）

---

## 設計判断

### 判断1: ショートカット設定のUI方式

**問題**: ユーザーがグローバルショートカットを変更するUIをどう実装するか

**選択肢**:
1. テキストフィールドにキーコンビネーションを表示 + 「Record」ボタンでキー入力をキャプチャ
2. ドロップダウンで修飾キーとキーを個別選択
3. 既存のOSSライブラリ（KeyboardShortcuts等）を使用

**決定**: 選択肢1（Record方式）

**理由**:
- 直感的な操作で、他のmacOSアプリ（システム環境設定等）と同じUXパターン
- 外部依存なしでSwift/AppKitのみで実装可能
- Phase 2で作成する設定ウィンドウ（`SettingsView`）にセクション追加するだけで済む

**トレードオフ**:
- メリット: ユーザーが慣れたUI、自由なキー設定が可能
- デメリット: キー入力キャプチャのハンドリングがやや複雑（NSEvent.addLocalMonitorHandler）

### 判断2: ページネーション方式

**問題**: 大量の履歴エントリをどのように段階的に読み込むか

**選択肢**:
1. オフセットベースのページネーション（`LIMIT ? OFFSET ?`）
2. カーソルベースのページネーション（`WHERE created_at < ? LIMIT ?`）
3. 全件取得に切り替え（メモリ内保持）

**決定**: 選択肢2（カーソルベース）

**理由**:
- `created_at DESC`でソート済みなので、最後のエントリのタイムスタンプをカーソルとして使える
- OFFSETベースより大量データで効率的（OFFSETはスキップ行を全てスキャンする）
- `idx_created_at`インデックスを活用でき、一貫したパフォーマンス

**トレードオフ**:
- メリット: 大量データでも安定した読み込み速度
- デメリット: 実装がやや複雑（カーソル値の管理が必要）

### 判断3: CI/CDのビルド環境

**問題**: Rust + Swift（Xcode）のクロスビルドをGitHub Actionsでどう実現するか

**選択肢**:
1. macOSランナー（`macos-latest`）でRustとXcodeを両方ビルド
2. Rustビルドをseparateジョブ（Linux）で実行し、成果物をmacOSジョブに渡す
3. Self-hostedランナーを使用

**決定**: 選択肢1（macOSランナー単体）

**理由**:
- swift-bridgeの生成コードがXcodeビルドに必要なため、Rustビルドとswift build は同一環境で実行する必要がある
- macOSランナーにはXcodeとHomebrewがプリインストール済み
- プロジェクト規模が小さく、ビルド時間はランナー1台で十分

**トレードオフ**:
- メリット: シンプルな構成、環境差異なし
- デメリット: macOSランナーはLinuxランナーより高コスト（GitHub Actions分単位課金）

### 判断4: リリースアーティファクトの形式

**問題**: ビルド成果物をどの形式で配布するか

**選択肢**:
1. `.app`をzip圧縮（`CB.app.zip`）
2. DMGイメージ（`CB.dmg`）
3. PKGインストーラー

**決定**: 選択肢1（zip圧縮）

**理由**:
- DMG作成は`create-dmg`等の外部ツールが必要だが、zipは`ditto`コマンドで生成可能
- ダウンロード後のダブルクリックで即使用可能
- Application Support等への特別なインストール処理が不要なアプリ構成

**トレードオフ**:
- メリット: シンプル、ツール不要、ユーザーが自由な場所に配置可能
- デメリット: DMGのようなインストールガイド（Applicationsフォルダへのドラッグ）がない

---

## 実装タスク

| タスクID | タスク名 | 説明 | 依存 | 状態 |
|---------|---------|------|------|------|
| 003-01 | ショートカットレコーダーUI | ShortcutRecorderView: Record方式キャプチャ、@AppStorage永続化 | Phase 2完了 | ✅ 完了 |
| 003-02 | ShortcutManager動的登録 | UserDefaults.didChangeNotification監視 → unregister/register | 003-01 | ✅ 完了 |
| 003-03 | ページネーションAPI（Rust） | get_entries_before(before_timestamp, limit) カーソルベース | - | ✅ 完了 |
| 003-04 | 無限スクロール（Swift） | loadMoreEntries() + .onAppear(末尾5件)でトリガー | 003-03 | ✅ 完了 |
| 003-05 | 画像キャッシュ最適化 | NSCache<NSNumber, NSImage>（countLimit:100, totalCostLimit:50MB） | - | ✅ 完了 |
| 003-06 | CIワークフロー | .github/workflows/ci.yml: rust-test → xcode-build | - | ✅ 完了 |
| 003-07 | バージョン管理・コード署名 | MARKETING_VERSION 1.0.0、CB.entitlements | - | ✅ 完了 |
| 003-08 | リリースワークフロー | .github/workflows/release.yml: v*タグ → ditto zip → GitHub Release | 003-06, 003-07 | ✅ 完了 |

**状態記号**:
- ⏳ 未着手
- 🏃 進行中
- ✅ 完了
- ⛔ ブロック中

---

## 詳細実装内容

### タスク003-01: ショートカットレコーダーUI

**目的**: ユーザーがグローバルショートカットを任意のキーコンビネーションに変更できるUIを提供する

**対象ファイル**:
- `CB/Sources/Views/ShortcutRecorderView.swift`: 新規作成
- `CB/Sources/Views/SettingsView.swift`: 修正（Phase 2で作成済みの想定）

**実装内容**:

1. `ShortcutRecorderView`を作成:
   - 現在のショートカットを表示するテキストフィールド（例: `⌥⌘V`）
   - 「Record」ボタン押下で入力待ち状態に遷移
   - `NSEvent.addLocalMonitorForEvents(matching: .keyDown)`でキー入力をキャプチャ
   - 修飾キー（⌘/⌃/⌥/⇧）+ メインキーの組み合わせを記録
   - 「Clear」ボタンでデフォルト（⌥⌘V）にリセット

2. ショートカットの永続化:
   - `UserDefaults`に`shortcutKeyCode: UInt32`と`shortcutModifiers: UInt32`を保存
   - `@AppStorage`で`SettingsView`と連動

3. バリデーション:
   - 修飾キーなしのショートカットは拒否（通常キー入力と競合するため）
   - 既知のシステムショートカット（⌘Q, ⌘W等）との競合を警告

---

### タスク003-02: ShortcutManager動的登録

**目的**: ショートカット設定変更時に、既存のホットキーを解除して新しいキーで再登録する

**対象ファイル**:
- `CB/Sources/ShortcutManager.swift`: 修正

**実装内容**:

1. `ShortcutManager`のリファクタリング:
   - `EventHotKeyRef`を保持するプロパティを追加
   - `registerHotKey(keyCode: UInt32, modifiers: UInt32)`メソッドを追加
   - `unregisterHotKey()`メソッドを追加（`UnregisterEventHotKey`呼び出し）
   - 初期化時にUserDefaultsから設定を読み取り（未設定ならデフォルト⌥⌘V）

2. 設定変更の監視:
   - `UserDefaults`の変更を`NotificationCenter`（`.NSUserDefaultsDidChange`）で監視
   - 変更検知時に`unregisterHotKey()` → `registerHotKey()`の順で再登録

3. デフォルト値:
   - keyCode: `kVK_ANSI_V`（0x09）
   - modifiers: `cmdKey | optionKey`

---

### タスク003-03: ページネーションAPI（Rust）

**目的**: カーソルベースのページネーションで大量履歴の段階的取得を可能にする

**対象ファイル**:
- `crates/cb-core/src/storage.rs`: 修正
- `crates/cb-core/src/lib.rs`: 修正

**実装内容**:

1. `get_entries_before()`関数を`storage.rs`に追加:
```rust
pub fn get_entries_before(
    &self,
    before_timestamp: i64,  // 0の場合は最新から
    limit: i32,
) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
    if before_timestamp == 0 {
        // 既存のget_recent_entriesと同等
        self.get_recent_entries(limit)
    } else {
        let mut stmt = self.conn.prepare(
            "SELECT id, content_type, text_content, image_data, source_app, created_at
             FROM clipboard_entries
             WHERE created_at < ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        )?;
        // ...
    }
}
```

2. FFIブリッジ関数を追加:
```rust
fn get_entries_before(before_timestamp: i64, limit: i32) -> String;
```

3. 既存の`get_recent_entries`は互換性のため維持（内部で`get_entries_before(0, limit)`を呼ぶ形にリファクタ可能）

**テスト**:
- カーソル指定で正しい範囲のエントリが返ること
- before_timestamp=0で最新エントリが返ること
- 境界値（エントリ0件、カーソルが最古より古い等）

---

### タスク003-04: 無限スクロール（Swift）

**目的**: 履歴パネルで下方向にスクロールすると追加エントリを自動読み込みする

**対象ファイル**:
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正
- `CB/Sources/Views/HistoryPanel.swift`: 修正

**実装内容**:

1. `HistoryViewModel`の変更:
   - `loadEntries()`を初回読み込み用に維持（`get_entries_before(0, 50)`）
   - `loadMoreEntries()`を追加: 現在の最古エントリの`created_at`をカーソルとして次のページを取得
   - `hasMore: Bool`フラグで追加読み込み可能かを管理
   - `isLoadingMore: Bool`フラグで二重読み込みを防止

2. `HistoryPanel`の変更:
   - リスト末尾に到達検知: 最後のアイテムが表示されたら`loadMoreEntries()`を呼び出し
   - `.onAppear`モディファイアをリスト末尾付近のアイテムに適用して検知
   - 読み込み中はスピナー（`ProgressView`）を表示

3. キーボードナビゲーションの対応:
   - ↓キーでリスト末尾を超えた場合も`loadMoreEntries()`をトリガー

---

### タスク003-05: 画像キャッシュ最適化

**目的**: 画像キャッシュのメモリ使用量を制限し、大量画像エントリでもメモリが肥大化しないようにする

**対象ファイル**:
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正

**実装内容**:

1. `imageCache`を`Dictionary<Int64, NSImage>`から`NSCache<NSNumber, NSImage>`に変更:
   - `countLimit`: 100（最大100枚のサムネイルを保持）
   - `totalCostLimit`: 50MB（画像データサイズをcostとして指定）
   - NSCacheはメモリプレッシャー時に自動的にエントリを破棄する

2. `loadEntries()`でのキャッシュクリアを廃止:
   - 現在は`loadEntries()`呼び出し時に全キャッシュをクリアしているが、NSCacheに変更後は不要
   - キャッシュヒットした画像は再取得せずに再利用

3. `loadImage(for:)`のキャッシュ確認ロジックを`NSCache`のAPIに合わせて更新

---

### タスク003-06: CIワークフロー

**目的**: Pull Request・pushイベントでRustテストとXcodeビルドを自動実行する

**対象ファイル**:
- `.github/workflows/ci.yml`: 新規作成

**実装内容**:

1. ワークフロー定義:
```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  rust-test:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace

  xcode-build:
    runs-on: macos-latest
    needs: rust-test
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: brew install xcodegen
      - run: xcodegen generate
      - run: cargo build --release -p cb-core
      - run: xcodebuild -project CB.xcodeproj -scheme CB build
```

2. キャッシュ戦略:
   - Rustの依存クレート: `Swatinem/rust-cache`
   - Homebrewパッケージ: ランナーにプリインストール済みのものを活用

---

### タスク003-07: バージョン管理・コード署名

**目的**: リリース配布に必要なバージョン番号管理とコード署名を設定する

**対象ファイル**:
- `project.yml`: 修正
- `CB/CB.entitlements`: 新規作成

**実装内容**:

1. `project.yml`にバージョン情報を追加:
```yaml
settings:
  MARKETING_VERSION: "1.0.0"
  CURRENT_PROJECT_VERSION: "1"
```

2. コード署名設定:
```yaml
settings:
  CODE_SIGN_IDENTITY: "-"  # ローカル開発用（ad-hoc署名）
  # リリース時はGitHub Secretsから証明書を注入
```

3. エンタイトルメントファイル（`CB.entitlements`）:
```xml
<key>com.apple.security.automation.apple-events</key>
<true/>
```
   - アクセシビリティ権限（CGEventによるキーシミュレート）に必要

4. `SettingsView`（Phase 2で作成済み）のバージョン表示を`MARKETING_VERSION`から取得するよう変更

---

### タスク003-08: リリースワークフロー

**目的**: Gitタグ（`v*`）のプッシュをトリガーに、リリースビルドとGitHub Release作成を自動化する

**対象ファイル**:
- `.github/workflows/release.yml`: 新規作成

**実装内容**:

1. ワークフロー定義:
```yaml
name: Release
on:
  push:
    tags: ["v*"]

jobs:
  build-and-release:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: brew install xcodegen
      - run: xcodegen generate
      - run: cargo build --release -p cb-core
      - run: xcodebuild -project CB.xcodeproj -scheme CB -configuration Release build
      - name: Package app
        run: |
          ditto -c -k --keepParent \
            build/Release/CB.app \
            CB-${GITHUB_REF_NAME}.zip
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: CB-*.zip
          generate_release_notes: true
```

2. リリースフロー:
   - `project.yml`の`MARKETING_VERSION`を更新
   - `git tag v1.0.0 && git push origin v1.0.0`
   - GitHub Actionsが自動でビルド → zip作成 → Release作成

3. リリースノート:
   - `generate_release_notes: true`でPRベースの自動生成を使用

---

## テスト計画

### 新規テスト（Rust）

| テスト種別 | テスト対象 | テスト数 | 内容 |
|-----------|-----------|---------|------|
| ユニット | `storage::get_entries_before` | 4個 | カーソルあり/なし、空DB、境界値（同一タイムスタンプ） |

### 既存テストへの影響

- [x] 既存テストの修正は不要（`get_recent_entries`は互換性維持）
- [x] 回帰テストの実行が必要（`cargo test --workspace`）

### 手動テスト

| テスト項目 | 確認内容 |
|-----------|---------|
| ショートカット変更 | 設定画面でキーを変更 → 新しいショートカットでパネルが開くこと |
| ショートカットリセット | 「Clear」押下 → デフォルト（⌥⌘V）に戻ること |
| 無限スクロール | 50件超の履歴 → スクロールで追加読み込みされること |
| 画像キャッシュ | 大量画像エントリ → メモリ使用量が一定範囲に収まること |
| CIワークフロー | PRを作成 → GitHub Actionsでテスト・ビルドが成功すること |
| リリース | タグをプッシュ → GitHub Releaseにzipが添付されること |

---

## 成功基準

**受け入れ条件**:
- [x] `cargo test --workspace` で全テストパス（ページネーションテスト含む）
- [x] Xcodeビルド・起動が成功
- [x] 設定画面でグローバルショートカットを変更でき、変更後のキーでパネルが開く
- [x] 50件を超える履歴がある場合、スクロールで追加エントリが自動読み込みされる
- [x] 画像キャッシュがNSCacheベースで動作し、メモリ制限が機能する
- [x] GitHub ActionsのCIワークフローがPR/pushで正常実行される
- [x] `v*`タグプッシュでGitHub Releaseが自動作成され、zipファイルが添付される

---

## 依存関係

### ブロックするタスク

- なし（Phase 3が最終フェーズ）

### ブロックされるタスク

- Phase 2: 生産性向上機能（✅ 完了）
  - `SettingsView`（002-06）は完了済み

### タスク間の依存

```
003-01 (ショートカットUI) → 003-02 (ShortcutManager動的登録)
003-03 (ページネーション Rust) → 003-04 (無限スクロール Swift)
003-05 (画像キャッシュ) は独立して実装可能
003-06 (CI) + 003-07 (バージョン管理) → 003-08 (リリース)
```

---

## リスクと緩和策

| リスク | 影響度 | 結果 |
|--------|--------|------|
| Carbon Event Managerの将来的な非推奨化 | 中 | ✅ macOS 26でも正常動作。動的再登録も問題なし |
| NSEvent.addLocalMonitorがNSPanel上で動作しない可能性 | 中 | ✅ Settings Scene（通常ウィンドウ）で問題なし |
| macOSランナーのXcodeバージョン不一致 | 中 | ✅ macos-latestランナーで正常動作 |
| コード署名なしのzipがGatekeeperでブロックされる | 高 | ⚠️ ad-hoc署名（`CODE_SIGN_IDENTITY: "-"`）。将来的にDeveloper IDで署名 |

---

## 関連ドキュメント

- [Phase 1: MVP計画](../resolved/001-mvp-initial-setup.md)
- [Phase 2: 生産性向上計画](../resolved/002-phase2-productivity.md)
- [技術スタック ADR](../design/decisions/001-technology-stack.md)
- [UIデザインシステム ADR](../design/decisions/002-ui-design-system.md)
- [cb-core モジュール設計](../design/modules/cb-core.md)
- [UI モジュール設計](../design/modules/ui.md)
- [ロードマップ](../status/roadmap.md)
- [実装ステータス](../status/implementation.md)
