import AppKit
import Combine
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "ClipboardMonitor")

@MainActor
class ClipboardMonitor: ObservableObject {
    @Published var latestEntryTimestamp: Date = Date()

    var skipNextChange = false

    private var timer: Timer?
    private var lastChangeCount: Int = 0
    private var lastContentHash: Int = 0
    private var isChecking = false

    init() {
        startMonitoring()
    }

    deinit {
        // Safe to access from deinit as the object is being destroyed
        MainActor.assumeIsolated {
            timer?.invalidate()
        }
    }

    func startMonitoring() {
        lastChangeCount = NSPasteboard.general.changeCount
        timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            Task { @MainActor in
                guard let self, !self.isChecking else { return }
                self.isChecking = true
                defer { self.isChecking = false }
                self.checkClipboard()
            }
        }
    }

    func stopMonitoring() {
        timer?.invalidate()
        timer = nil
    }

    private func checkClipboard() {
        let pasteboard = NSPasteboard.general
        let currentCount = pasteboard.changeCount

        guard currentCount != lastChangeCount else { return }
        lastChangeCount = currentCount

        if skipNextChange {
            skipNextChange = false
            return
        }

        let sourceApp = NSWorkspace.shared.frontmostApplication?.localizedName ?? ""

        if let string = pasteboard.string(forType: .string) {
            let hash = string.hashValue
            guard hash != lastContentHash else { return }
            lastContentHash = hash

            let isFilePath: Bool = {
                let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
                // 複数行テキストはパスではない
                guard !trimmed.contains("\n") else { return false }
                // ~ で始まる場合はチルダ展開
                let expanded = trimmed.hasPrefix("~")
                    ? NSString(string: trimmed).expandingTildeInPath
                    : trimmed
                guard expanded.hasPrefix("/") else { return false }
                // ファイルまたは親ディレクトリが存在すればパスと判定
                let fm = FileManager.default
                return fm.fileExists(atPath: expanded)
                    || fm.fileExists(atPath: (expanded as NSString).deletingLastPathComponent)
            }()
            let contentType = isFilePath ? "FilePath" : "PlainText"

            Task.detached {
                let success = save_clipboard_entry(contentType, string, sourceApp)
                if !success {
                    await MainActor.run {
                        logger.error("Failed to save clipboard entry (type: \(contentType))")
                    }
                }
            }
            latestEntryTimestamp = Date()
        } else if let imageData = pasteboard.data(forType: .tiff) ?? pasteboard.data(forType: .png) {
            let hash = imageData.hashValue
            guard hash != lastContentHash else { return }
            lastContentHash = hash

            Task.detached {
                imageData.withUnsafeBytes { rawBuffer in
                    let buffer = rawBuffer.bindMemory(to: UInt8.self)
                    let success = save_clipboard_image(
                        UnsafeBufferPointer(start: buffer.baseAddress, count: buffer.count),
                        sourceApp
                    )
                    if !success {
                        Task { @MainActor in
                            logger.error("Failed to save clipboard image")
                        }
                    }
                }
            }
            latestEntryTimestamp = Date()
        }
    }
}
