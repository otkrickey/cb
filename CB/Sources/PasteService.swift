import AppKit
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "PasteService")

@MainActor
enum PasteService {
    /// エントリを NSPasteboard に載せる。外部化されたエントリの blob 読み込みは
    /// `Task.detached` で MainActor の外に退避してから戻ってきてペーストボードを触る。
    /// これにより MainActor 上での同期ディスク I/O によるフリーズを回避する
    /// (PR #18 review 指摘)。
    static func copyToClipboard(
        entry: ClipboardEntryModel,
        imageData preloadedImageData: Data? = nil,
        monitor: ClipboardMonitor,
        asPlainText: Bool = false
    ) async {
        // 事前に必要な情報を吸い出して Sendable な形にする (Task.detached クロージャに渡すため)
        let id = entry.id
        let isImage = entry.isImage
        let isExternalized = entry.isExternalized
        let inlineText = entry.textContent
        let textPreview = entry.textPreview
        let blobSha = entry.blobSha256

        // ─── 1. バックグラウンドで FFI (blob 読み込みを含む) を実行 ───
        let payload: Payload = await Task.detached {
            if asPlainText {
                return .text(loadFullText(id: id, inline: inlineText, isExternalized: isExternalized, preview: textPreview))
            }
            if isImage {
                let data = preloadedImageData ?? loadImage(id: id)
                let fallback = loadFullText(id: id, inline: inlineText, isExternalized: isExternalized, preview: textPreview)
                return .image(data: data, textFallback: fallback)
            }
            return .text(loadFullText(id: id, inline: inlineText, isExternalized: isExternalized, preview: textPreview))
        }.value

        // blob 欠損の検出もバックグラウンドで
        let blobMissing = isExternalized ? await Task.detached { is_blob_missing(id) }.value : false
        if blobMissing {
            logger.warning("blob missing for entry \(id) (sha=\(blobSha ?? "?")); pasted preview fallback")
        }

        // ─── 2. MainActor に戻って NSPasteboard を触る (ここは同期・軽量) ───
        monitor.skipNextChange = true
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        switch payload {
        case .text(let text):
            if let text { pasteboard.setString(text, forType: .string) }
        case .image(let data, let textFallback):
            if let data, let nsImage = NSImage(data: data) {
                pasteboard.writeObjects([nsImage])
            } else if let text = textFallback {
                pasteboard.setString(text, forType: .string)
            }
        }
    }

    private enum Payload {
        case text(String?)
        case image(data: Data?, textFallback: String?)
    }

    static func simulatePaste() {
        guard AXIsProcessTrusted() else {
            logger.warning("Accessibility permission not granted")
            return
        }

        let source = CGEventSource(stateID: .hidSystemState)
        guard let keyDown = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: true),
              let keyUp = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: false) else {
            logger.error("Failed to create CGEvent for paste simulation")
            return
        }

        keyDown.flags = .maskCommand
        keyUp.flags = .maskCommand

        keyDown.post(tap: .cghidEventTap)
        keyUp.post(tap: .cghidEventTap)
    }
}

// ─────────────────────────────────────────────
// nonisolated ヘルパ (Task.detached から呼べる)
// ─────────────────────────────────────────────

/// entry 本体からフルテキストを取得する。優先順:
/// 1. inline `text_content` (JSON 由来、小さいエントリ)
/// 2. FFI `get_entry_text` (blob 読み込み or Rust 側 preview fallback)
/// 3. `text_preview` (最終 fallback)
private func loadFullText(id: Int64, inline: String?, isExternalized: Bool, preview: String?) -> String? {
    if let inline { return inline }
    if isExternalized, let rustStr = get_entry_text(id) {
        return rustStr.toString()
    }
    return preview
}

private func loadImage(id: Int64) -> Data? {
    guard let rustVec = get_entry_image(id) else { return nil }
    return Data(bytes: rustVec.as_ptr(), count: rustVec.len())
}
