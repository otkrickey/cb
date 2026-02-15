import AppKit
import Carbon
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "ShortcutManager")

// Carbon hotkey callback needs a global reference
nonisolated(unsafe) private var _shortcutManagerInstance: ShortcutManager?

@MainActor
class ShortcutManager {
    nonisolated(unsafe) private var hotKeyRef: EventHotKeyRef?
    nonisolated(unsafe) private var defaultsObserver: NSObjectProtocol?
    nonisolated(unsafe) private var eventHandlerInstalled = false
    var onTogglePanel: (() -> Void)?

    init() {
        // Observe UserDefaults changes to re-register hotkey
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.reregister()
        }
    }

    deinit {
        if let observer = defaultsObserver {
            NotificationCenter.default.removeObserver(observer)
        }
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
        }
        _shortcutManagerInstance = nil
    }

    func start() {
        // Read shortcut settings from UserDefaults
        let keyCode = UserDefaults.standard.integer(forKey: "shortcutKeyCode")
        let modifiers = UserDefaults.standard.integer(forKey: "shortcutModifiers")

        // Use defaults if not set
        let finalKeyCode = keyCode != 0 ? UInt32(keyCode) : UInt32(kVK_ANSI_V)
        let finalModifiers = modifiers != 0 ? UInt32(modifiers) : UInt32(cmdKey | optionKey)

        logger.notice("Registering global hotkey with keyCode: \(finalKeyCode), modifiers: \(finalModifiers)")
        _shortcutManagerInstance = self

        // Install Carbon event handler only once
        if !eventHandlerInstalled {
            var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
            InstallEventHandler(GetApplicationEventTarget(), hotKeyHandler, 1, &eventType, nil, nil)
            eventHandlerInstalled = true
        }

        // Register hotkey
        let hotKeyID = EventHotKeyID(signature: OSType(0x43425F56), id: 1) // "CB_V"
        let status = RegisterEventHotKey(finalKeyCode, finalModifiers, hotKeyID, GetApplicationEventTarget(), 0, &hotKeyRef)

        if status == noErr {
            logger.notice("Global hotkey registered successfully")
        } else {
            logger.error("Failed to register hotkey: \(status)")
        }
    }

    func stop() {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
        }
        hotKeyRef = nil
    }

    func reregister() {
        logger.notice("Re-registering hotkey due to settings change")
        stop()
        start()
    }

    fileprivate func handleHotKey() {
        logger.notice("Hotkey pressed!")
        onTogglePanel?()
    }
}

private func hotKeyHandler(nextHandler: EventHandlerCallRef?, event: EventRef?, userData: UnsafeMutableRawPointer?) -> OSStatus {
    DispatchQueue.main.async {
        Task { @MainActor in
            _shortcutManagerInstance?.handleHotKey()
        }
    }
    return noErr
}
