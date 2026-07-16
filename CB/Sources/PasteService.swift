import AppKit
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "PasteService")

@MainActor
enum PasteService {
    static func copyToClipboard(entry: ClipboardEntryModel, imageData: Data? = nil, monitor: ClipboardMonitor, asPlainText: Bool = false) {
        monitor.skipNextChange = true

        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()

        if asPlainText {
            if let text = fullText(for: entry) {
                pasteboard.setString(text, forType: .string)
            }
        } else if entry.isImage {
            let data = imageData ?? loadImageData(for: entry.id)
            if let data, let nsImage = NSImage(data: data) {
                pasteboard.writeObjects([nsImage])
            } else if let text = fullText(for: entry) {
                pasteboard.setString(text, forType: .string)
            }
        } else if let text = fullText(for: entry) {
            pasteboard.setString(text, forType: .string)
        }

        if entry.isExternalized, is_blob_missing(entry.id) {
            logger.warning("blob missing for entry \(entry.id) (sha=\(entry.blobSha256 ?? "?")); pasted preview fallback")
        }
    }

    /// entry 本体からフルテキストを取得する。優先順:
    /// 1. inline `text_content` (JSON 由来、小さいエントリ)
    /// 2. FFI `get_entry_text` (blob 読み込み or Rust 側 preview fallback)
    /// 3. `text_preview` (最終 fallback)
    private static func fullText(for entry: ClipboardEntryModel) -> String? {
        if let inline = entry.textContent { return inline }
        if entry.isExternalized {
            if let rustStr = get_entry_text(entry.id) {
                return rustStr.toString()
            }
        }
        return entry.textPreview
    }

    private static func loadImageData(for id: Int64) -> Data? {
        guard let rustVec = get_entry_image(id) else { return nil }
        return Data(bytes: rustVec.as_ptr(), count: rustVec.len())
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
