import Foundation
import CryptoKit
import Security
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "KeychainManager")

enum KeychainManager {
    private static let service = "com.otkrickey.cb.db-encryption"
    private static let account = "clipboard-db-key"

    static func getOrCreateKey() -> String? {
        if let existing = getKey() {
            return existing
        }

        let key = SymmetricKey(size: .bits256)
        let keyData = key.withUnsafeBytes { Data($0) }
        let keyString = keyData.base64EncodedString()

        if saveKey(keyString) {
            logger.notice("Generated and saved new encryption key to Keychain")
            return keyString
        }

        logger.error("Failed to save encryption key to Keychain")
        return nil
    }

    private static func getKey() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess, let data = result as? Data else {
            return nil
        }

        return String(data: data, encoding: .utf8)
    }

    private static func saveKey(_ key: String) -> Bool {
        guard let data = key.data(using: .utf8) else { return false }

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
            kSecAttrSynchronizable as String: false,
        ]

        let status = SecItemAdd(query as CFDictionary, nil)
        return status == errSecSuccess
    }
}
