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
            // cb-core 側の migrate_to_encrypted は「encrypted_path は存在しないこと」
            // を前提とする。ただしここでの dbPath 事前削除は「稼働中の完全な
            // 暗号化DB」を誤って消すリスクがあるので、rename でバックアップに
            // 退避してから移行し、失敗時にロールバックできるようにする
            // (PR #15 review round 10 指摘)。
            var backupPath: String? = nil
            if fileManager.fileExists(atPath: dbPath) {
                let candidate = "\(dbPath).migration-backup-\(Int(Date().timeIntervalSince1970))"
                do {
                    try fileManager.moveItem(atPath: dbPath, toPath: candidate)
                    backupPath = candidate
                    logger.warning("Moved existing encrypted DB aside for migration retry: \(candidate)")
                } catch {
                    logger.error("Failed to move existing encrypted DB aside: \(error). Aborting migration to avoid data loss.")
                    return
                }
            }
            let migrated = migrate_database(plainPath, dbPath, encryptionKey)
            if migrated {
                logger.notice("Successfully migrated plain DB to encrypted DB")
                do {
                    try fileManager.removeItem(atPath: plainPath)
                    logger.notice("Removed old plain DB")
                } catch {
                    logger.error("Failed to remove old plain DB: \(error)")
                }
                if let backup = backupPath {
                    // 新しい暗号化 DB が正常に作れたので古い残骸バックアップは破棄
                    try? fileManager.removeItem(atPath: backup)
                }
            } else {
                logger.error("Database migration failed")
                // ロールバック: バックアップを dbPath に戻し、plainPath はそのまま残す
                if let backup = backupPath {
                    do {
                        try fileManager.moveItem(atPath: backup, toPath: dbPath)
                        logger.notice("Rolled back to previous encrypted DB from backup")
                    } catch {
                        logger.error("Failed to roll back encrypted DB backup at \(backup): \(error). Manual recovery required.")
                    }
                }
            }
        }
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
