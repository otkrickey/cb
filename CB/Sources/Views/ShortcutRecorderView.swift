import SwiftUI
import AppKit
import Carbon

struct ShortcutRecorderView: View {
    @AppStorage("shortcutKeyCode") private var keyCode: Int = 0x09 // kVK_ANSI_V
    @AppStorage("shortcutModifiers") private var modifiers: Int = 0x0900 // cmdKey | optionKey

    @State private var isRecording = false
    @State private var eventMonitor: Any?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("履歴パネルを開く")
                Spacer()

                Button(action: {
                    if isRecording {
                        stopRecording()
                    } else {
                        startRecording()
                    }
                }) {
                    Text(isRecording ? "Done" : shortcutString)
                        .frame(minWidth: 80)
                }
                .buttonStyle(.bordered)

                Button("Reset") {
                    keyCode = 0x09 // kVK_ANSI_V
                    modifiers = 0x0900 // cmdKey | optionKey
                }
                .buttonStyle(.borderless)
            }

            if isRecording {
                Text("Press a key combination...")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .onDisappear {
            stopRecording()
        }
    }

    private var shortcutString: String {
        ShortcutFormatter.formatShortcut(keyCode: keyCode, modifiers: modifiers)
    }

    private func startRecording() {
        isRecording = true

        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let newModifiers = event.modifierFlags.carbonModifiers
            let newKeyCode = event.keyCode

            // Require at least one modifier key
            if newModifiers == 0 {
                NSSound.beep()
                return nil
            }

            // Update stored values
            keyCode = Int(newKeyCode)
            modifiers = Int(newModifiers)

            stopRecording()
            return nil
        }
    }

    private func stopRecording() {
        isRecording = false
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
            eventMonitor = nil
        }
    }
}

// Helper extension to convert NSEvent.ModifierFlags to Carbon modifiers
extension NSEvent.ModifierFlags {
    var carbonModifiers: UInt32 {
        var result: UInt32 = 0
        if contains(.control) {
            result |= UInt32(controlKey)
        }
        if contains(.option) {
            result |= UInt32(optionKey)
        }
        if contains(.shift) {
            result |= UInt32(shiftKey)
        }
        if contains(.command) {
            result |= UInt32(cmdKey)
        }
        return result
    }
}
