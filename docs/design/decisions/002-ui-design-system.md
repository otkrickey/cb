<!--
種別: decisions
対象: UIデザインシステム選定
作成日: 2026-02-15
更新日: 2026-02-16
担当: AIエージェント
-->

# UIデザインシステム — Liquid Glass

## 概要

cbプロジェクトのUIデザインシステムとして、macOS Tahoe（macOS 26）で導入された Liquid Glass を採用する。

---

## 設計判断

### 決定内容

Apple の Liquid Glass デザインシステムを採用し、macOS Tahoe ネイティブの外観・操作体験を提供する。

### 理由

- macOS Tahoe のデザイン言語に準拠し、OSとの統一感を実現する
- `.glassEffect` modifier 等のネイティブAPIにより実装コストが低い
- コンテンツファーストの設計原則がクリップボードマネージャーのUXに適合する

### 代替案

| 案 | 概要 | 不採用理由 |
|----|------|-----------|
| カスタムデザイン | 独自のUI言語を設計 | macOSとの統一感が失われる。デザインコスト大 |
| 従来のmacOSスタイル | Big Sur以降のフラットデザイン | macOS Tahoe上で旧世代の見た目になる |

### トレードオフ

- **利点**: OS統一感、ネイティブAPI活用、コンテンツファースト、自動ダークモード対応
- **欠点**: macOS 26+ 限定、旧macOSでの動作不可

---

## 実際の適用

### パネルウィンドウ

`KeyablePanel`（`NSPanel`サブクラス）をフローティングウィンドウとして使用:
- `borderless` + `fullSizeContentView` スタイル
- 背景透明（`backgroundColor = .clear`, `isOpaque = false`）
- `GlassEffectContainer`でSwiftUIコンテンツ全体をラップ
- `.glassEffect(.regular, in: .rect(cornerRadius: 16))` を外枠に適用

```swift
GlassEffectContainer {
    VStack(spacing: 0) { ... }
        .frame(width: 720, height: 480)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .glassEffect(.regular, in: .rect(cornerRadius: 16))
}
```

### レイアウト構成

二ペインレイアウト（720x480）:

```
┌─────────────────────────────────────────────────┐
│  🔍 Search bar                                   │
├──────────────┬──────────────────────────────────┤
│              │                                    │
│  Entry List  │  Detail Preview                    │
│  (280pt)     │  (text: monospace / image: fit)    │
│              │                                    │
│              ├──────────────────────────────────┤
│              │  Information                       │
│              │  Source / Type / Size / Date        │
├──────────────┴──────────────────────────────────┤
│  📋 Clipboard History        Paste to {App} ↩    │
└─────────────────────────────────────────────────┘
```

### リストアイテム

`ClipboardItemRow`の選択状態は `.selection` の背景色で表現:
- SF Symbolアイコン（`doc.text.fill` / `photo` / `folder.fill` / `textformat`）
- 1行プレビューテキスト（`lineLimit(1)`）
- 日付グループヘッダ（"Today" / "Yesterday" / フォーマット済み日付）

### 詳細プレビュー（Detail Preview）

右ペインの詳細プレビューエリア:
- テキストエントリ: `.textSelection(.enabled)` で部分選択・⌘+Cコピーに対応
- 画像エントリ: `aspectRatio(contentMode: .fit)` でフィット表示
- Information セクション: Source / Content type / Characters / Lines / Size / Copied

**フォーカス制御**:
- サーチフィールドはデフォルトでフォーカスを保持
- ユーザーがプレビューテキストをクリックした場合のみフォーカス解除を許可（テキスト選択のため）
- リストナビゲーション（↑↓キー）時にサーチフォーカスを自動復帰

### ボトムバー

`.glassEffect(.regular, in: .rect(cornerRadius: 4))` をReturnキーインジケータに適用し、ガラスの質感をアクセントに使用。

---

## 関連ドキュメント

- [技術スタック ADR](./001-technology-stack.md)
- [初期仕様書](../../archive/initial_plan.md)

## 参考資料

- [Liquid Glass | Apple Developer Documentation](https://developer.apple.com/documentation/TechnologyOverviews/liquid-glass)
- [Build a SwiftUI app with the new design — WWDC25 Session 323](https://developer.apple.com/videos/play/wwdc2025/323/)
