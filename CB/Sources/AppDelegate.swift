import AppKit
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "AppDelegate")

class AppDelegate: NSObject, NSApplicationDelegate {
    var monitor: ClipboardMonitor?
    private var shortcutManager: ShortcutManager?
    private(set) var historyWindowController: HistoryWindowController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        logger.notice("applicationDidFinishLaunching started")
        initStorage()

        Task { @MainActor in
            let mon = ClipboardMonitor()
            self.monitor = mon

            let windowController = HistoryWindowController(monitor: mon)
            self.historyWindowController = windowController

            let shortcut = ShortcutManager()
            shortcut.onTogglePanel = { [weak windowController] in
                logger.notice("Toggle panel called")
                Task { @MainActor in
                    windowController?.toggle()
                }
            }
            shortcut.start()
            self.shortcutManager = shortcut
            logger.notice("Shortcut manager started")

            // Check accessibility permission
            self.checkAccessibilityPermission()
        }
    }

    private func initStorage() {
        guard let appSupportURL = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first else {
            logger.error("Failed to get Application Support directory")
            return
        }
        let appSupportDir = appSupportURL.appendingPathComponent("CB")

        do {
            try FileManager.default.createDirectory(
                at: appSupportDir,
                withIntermediateDirectories: true
            )
        } catch {
            logger.error("Failed to create app support directory: \(error)")
        }

        guard let encryptionKey = KeychainManager.getOrCreateKey() else {
            logger.error("Failed to obtain encryption key from Keychain")
            return
        }

        let dbPath = appSupportDir.appendingPathComponent("clipboard.db").path
        let plainPath = appSupportDir.appendingPathComponent("clipboard_plain.db").path
        let fm = FileManager.default

        // Migrate existing plain DB to encrypted if needed
        migrateToEncryptedDatabase(dbPath: dbPath, plainPath: plainPath, encryptionKey: encryptionKey, fileManager: fm)

        let success = init_storage(dbPath, encryptionKey)
        if success {
            logger.notice("Encrypted storage initialized at: \(dbPath)")
            // Cleanup old entries based on retention setting
            let retentionDays = UserDefaults.standard.integer(forKey: "retentionDays")
            let maxAge = retentionDays > 0 ? retentionDays : 7
            let deletedCount = cleanup_old_entries(Int32(maxAge))
            if deletedCount > 0 {
                logger.notice("Cleaned up \(deletedCount) old clipboard entries")
            }
        } else {
            logger.error("Failed to initialize storage")
        }
    }

    private func migrateToEncryptedDatabase(dbPath: String, plainPath: String, encryptionKey: String, fileManager: FileManager) {
        if fileManager.fileExists(atPath: dbPath) && !fileManager.fileExists(atPath: plainPath) {
            // Check if current DB is already encrypted by trying to open without key
            // If it opens fine, it's still a plain DB and needs migration
            if isPlainDatabase(dbPath) {
                do {
                    try fileManager.moveItem(atPath: dbPath, toPath: plainPath)
                    logger.notice("Renamed plain DB for migration")
                } catch {
                    logger.error("Failed to rename plain DB: \(error)")
                }
            }
        }

        if fileManager.fileExists(atPath: plainPath) {
            // 曖昧なケース: plainPath と dbPath が両方存在する場合。
            // (A) 前回中断/クラッシュで作られた不完全な残骸 = 削除して migrate すべき
            // (B) migrate 成功後にユーザが履歴を追加し、稼働中の暗号化 DB がここにある
            //     + plainPath 削除だけが一時 I/O エラーで残った = **稼働 DB を消してはいけない**
            //
            // ヘッダを見て平文 SQLite 由来か SQLCipher 由来かを判別する。
            // - "SQLite format 3\0" で始まる or 極端に小さい → (A) 残骸
            // - それ以外の非平文ヘッダ + 妥当なサイズ → (B) 稼働中の暗号化 DB
            // (B) の場合は plainPath を stale として削除し、migrate をスキップする。
            if fileManager.fileExists(atPath: dbPath) {
                if isEncryptedDatabase(dbPath) {
                    logger.warning("Both plain and encrypted DB exist; encrypted DB looks live. Treating plain DB as stale leftover from a previous successful migration.")
                    do {
                        try fileManager.removeItem(atPath: plainPath)
                        logger.notice("Removed stale plain DB after previous migration")
                    } catch {
                        logger.error("Failed to remove stale plain DB: \(error). Manual cleanup recommended.")
                    }
                    return
                }
                // (A) 残骸: 万一のために rename で退避してから migrate 試行。
                let backup = "\(dbPath).migration-backup-\(Int(Date().timeIntervalSince1970))"
                do {
                    try fileManager.moveItem(atPath: dbPath, toPath: backup)
                    logger.warning("Moved suspected stale encrypted DB aside: \(backup)")
                } catch {
                    logger.error("Failed to move suspected stale encrypted DB aside: \(error). Aborting migration to avoid data loss.")
                    return
                }
                let migrated = migrate_database(plainPath, dbPath, encryptionKey)
                if migrated {
                    logger.notice("Successfully re-migrated after moving stale encrypted DB aside")
                    try? fileManager.removeItem(atPath: plainPath)
                    try? fileManager.removeItem(atPath: backup)
                } else {
                    logger.error("Migration failed after aside; rolling back")
                    do {
                        try fileManager.moveItem(atPath: backup, toPath: dbPath)
                    } catch {
                        logger.error("Failed to roll back backup at \(backup): \(error). Manual recovery required.")
                    }
                }
                return
            }
            // 通常ケース: 平文 DB のみ、暗号化 DB は存在しない
            let migrated = migrate_database(plainPath, dbPath, encryptionKey)
            if migrated {
                logger.notice("Successfully migrated plain DB to encrypted DB")
                do {
                    try fileManager.removeItem(atPath: plainPath)
                    logger.notice("Removed old plain DB")
                } catch {
                    logger.error("Failed to remove old plain DB: \(error)")
                }
            } else {
                logger.error("Database migration failed")
            }
        }
    }

    /// 平文 SQLite ではなく、かつサイズが有意義な (妥当な暗号化 DB とみなせる)
    /// ファイルかを判定する。厳密な validity check ではないが、
    /// 「稼働中の SQLCipher DB」と「破損した残骸」を実用的に区別できる。
    private func isEncryptedDatabase(_ path: String) -> Bool {
        let fm = FileManager.default
        let size = (try? fm.attributesOfItem(atPath: path)[.size] as? Int) ?? 0
        guard size >= 1024 else { return false }
        guard let handle = FileHandle(forReadingAtPath: path) else { return false }
        defer { handle.closeFile() }
        let header = handle.readData(ofLength: 16)
        guard let plainMarker = "SQLite format 3".data(using: .utf8) else { return false }
        // 平文ヘッダで始まっていれば暗号化ではない
        return !header.starts(with: plainMarker)
    }

    private func isPlainDatabase(_ path: String) -> Bool {
        guard let handle = FileHandle(forReadingAtPath: path) else { return false }
        defer { handle.closeFile() }
        let header = handle.readData(ofLength: 16)
        // Plain SQLite files start with "SQLite format 3\0"
        guard let sqliteHeader = "SQLite format 3".data(using: .utf8) else { return false }
        return header.starts(with: sqliteHeader)
    }

    private func checkAccessibilityPermission() {
        guard !AXIsProcessTrusted() else { return }

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(1))

            let alert = NSAlert()
            alert.messageText = "アクセシビリティ権限が必要です"
            alert.informativeText = "CBがクリップボード内容をペーストするには、アクセシビリティ権限が必要です。システム設定で許可してください。"
            alert.alertStyle = .warning
            alert.addButton(withTitle: "システム設定を開く")
            alert.addButton(withTitle: "後で")

            let response = alert.runModal()
            if response == .alertFirstButtonReturn {
                let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
                AXIsProcessTrustedWithOptions(options)
            }
        }
    }
}
