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
            // Plain text mode: only set text as .string, ignore images
            if let text = entry.textContent {
                pasteboard.setString(text, forType: .string)
            }
        } else {
            // Normal mode: preserve original format
            if entry.isImage, let data = imageData, let nsImage = NSImage(data: data) {
                pasteboard.writeObjects([nsImage])
            } else if let text = entry.textContent {
                pasteboard.setString(text, forType: .string)
            }
        }
    }

    static func simulatePaste() {
        // Accessibility permission required for CGEvent
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
