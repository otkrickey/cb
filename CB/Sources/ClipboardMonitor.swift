import AppKit
import Combine
import CryptoKit
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "ClipboardMonitor")

// cb-core と揃える閾値。変更する場合は crates/cb-core/src/storage.rs と
// crates/cb-core/src/models.rs の const も揃えること。
private let TEXT_EXTERNALIZE_THRESHOLD_BYTES = 262_144
private let PREVIEW_BYTES_LIMIT = 8_192

@MainActor
class ClipboardMonitor: ObservableObject {
    @Published var latestEntryTimestamp: Date = Date()

    var skipNextChange = false

    private var timer: Timer?
    private var lastChangeCount: Int = 0
    private var lastContentSha: String = ""
    private var isChecking = false
    private lazy var blobDir: URL = {
        let path = blob_dir_path().toString()
        let url = URL(fileURLWithPath: path)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }()

    init() {
        startMonitoring()
    }

    deinit {
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
                await self.checkClipboard()
                self.isChecking = false
            }
        }
    }

    func stopMonitoring() {
        timer?.invalidate()
        timer = nil
    }

    private func checkClipboard() async {
        let pasteboard = NSPasteboard.general
        let currentCount = pasteboard.changeCount

        guard currentCount != lastChangeCount else { return }
        lastChangeCount = currentCount

        if skipNextChange {
            skipNextChange = false
            return
        }

        let sourceApp = NSWorkspace.shared.frontmostApplication?.localizedName ?? ""

        // NSPasteboard は main-actor 縛りなので Data 取得だけは main で行う。
        // それ以降 (hash / blob 書込み / FFI 呼び出し) はすべて detached。
        if let stringData = pasteboard.data(forType: .string) {
            await handleTextData(stringData, sourceApp: sourceApp)
        } else if let imageData = pasteboard.data(forType: .tiff) ?? pasteboard.data(forType: .png) {
            await handleImageData(imageData, sourceApp: sourceApp)
        }
    }

    private func handleTextData(_ data: Data, sourceApp: String) async {
        let byteSize = data.count
        let sha = await Task.detached { sha256Hex(data: data) }.value
        guard sha != lastContentSha else { return }
        lastContentSha = sha

        let preview = utf8Prefix(data: data, byteLimit: PREVIEW_BYTES_LIMIT)
        let isLarge = byteSize > TEXT_EXTERNALIZE_THRESHOLD_BYTES

        if isLarge {
            // 大きなテキストはフル String 化せず Data のまま blob に書き出す。
            // FFI に渡すのは preview (8KB) + sha256 のみ。
            let blobDir = self.blobDir
            let ok = await Task.detached {
                writeBlob(dir: blobDir, sha: sha, data: data)
                return save_clipboard_blob_ref("PlainText", preview, sha, Int64(byteSize), sourceApp)
            }.value
            if !ok { logger.error("Failed to save large text as blob ref (bytes=\(byteSize))") }
        } else {
            // 小さいテキストは UTF-8 デコードして inline 保存 (path 判定もここで)。
            let fullText = String(data: data, encoding: .utf8) ?? preview
            let contentType = detectFilePath(fullText) ? "FilePath" : "PlainText"
            let ok = await Task.detached {
                return save_clipboard_text_v2(contentType, preview, fullText, "", Int64(byteSize), sourceApp)
            }.value
            if !ok { logger.error("Failed to save small text (bytes=\(byteSize))") }
        }
        latestEntryTimestamp = Date()
    }

    private func handleImageData(_ data: Data, sourceApp: String) async {
        let byteSize = data.count
        let sha = await Task.detached { sha256Hex(data: data) }.value
        guard sha != lastContentSha else { return }
        lastContentSha = sha

        let blobDir = self.blobDir
        let ok = await Task.detached {
            writeBlob(dir: blobDir, sha: sha, data: data)
            return save_clipboard_blob_ref("Image", "", sha, Int64(byteSize), sourceApp)
        }.value
        if !ok { logger.error("Failed to save image as blob ref (bytes=\(byteSize))") }
        latestEntryTimestamp = Date()
    }
}

// ─────────────────────────────────────────────
// ヘルパ (nonisolated, background-safe)
// ─────────────────────────────────────────────

private func sha256Hex(data: Data) -> String {
    let digest = SHA256.hash(data: data)
    return digest.reduce(into: "") { $0.append(String(format: "%02x", $1)) }
}

/// 先頭 byteLimit バイトを UTF-8 として有効な範囲に切り詰めて String 化する。
/// マルチバイトの途中で切れた場合は境界まで戻す。
private func utf8Prefix(data: Data, byteLimit: Int) -> String {
    if data.count <= byteLimit {
        return String(data: data, encoding: .utf8) ?? ""
    }
    var slice = data.prefix(byteLimit)
    while !slice.isEmpty {
        if let s = String(data: slice, encoding: .utf8) {
            return s
        }
        slice = slice.dropLast()
    }
    return ""
}

/// <dir>/<sha>.bin へ書き込む。同一 sha のファイルが既にあれば no-op (dedup)。
private func writeBlob(dir: URL, sha: String, data: Data) {
    let url = dir.appendingPathComponent("\(sha).bin")
    let fm = FileManager.default
    if fm.fileExists(atPath: url.path) { return }
    try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
    let tmp = url.appendingPathExtension("tmp")
    do {
        try data.write(to: tmp, options: .atomic)
        try fm.moveItem(at: tmp, to: url)
    } catch {
        try? fm.removeItem(at: tmp)
        logger.error("Failed to write blob \(sha): \(error.localizedDescription)")
    }
}

private func detectFilePath(_ string: String) -> Bool {
    let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.contains("\n") else { return false }
    let expanded = trimmed.hasPrefix("~")
        ? NSString(string: trimmed).expandingTildeInPath
        : trimmed
    guard expanded.hasPrefix("/") else { return false }
    let fm = FileManager.default
    return fm.fileExists(atPath: expanded)
        || fm.fileExists(atPath: (expanded as NSString).deletingLastPathComponent)
}
