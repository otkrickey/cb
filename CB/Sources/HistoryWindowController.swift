import AppKit
import SwiftUI
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "HistoryWindowController")

@MainActor
class HistoryWindowController {
    private var panel: KeyablePanel?
    private let monitor: ClipboardMonitor
    private let selectionState = SelectionState()
    private let viewModel = HistoryViewModel()
    private var previousApp: NSRunningApplication?

    init(monitor: ClipboardMonitor) {
        self.monitor = monitor
    }

    var isVisible: Bool {
        panel?.isVisible ?? false
    }

    func toggle() {
        if let panel, panel.isVisible {
            logger.notice("toggle: panel visible → cycling type filter")
            viewModel.cycleTypeFilter()
            selectionState.selectedIndex = 0
            selectionState.entryCount = viewModel.filteredEntries.count
        } else {
            logger.notice("toggle: panel not visible → showing")
            show()
        }
    }

    func show() {
        if panel == nil {
            logger.notice("show: creating panel")
            createPanel()
        }
        guard let panel else {
            logger.error("show: panel creation failed")
            return
        }

        previousApp = NSWorkspace.shared.frontmostApplication
        viewModel.targetAppName = previousApp?.localizedName ?? ""
        logger.notice("show: previousApp=\(self.previousApp?.localizedName ?? "nil")")

        viewModel.searchText = ""
        viewModel.typeFilter = .all
        viewModel.loadEntries()
        selectionState.selectedIndex = 0
        selectionState.entryCount = viewModel.filteredEntries.count
        logger.notice("show: loaded \(self.viewModel.filteredEntries.count) entries")

        let panelWidth: CGFloat = 720
        let panelHeight: CGFloat = 480

        if let screen = NSScreen.main {
            let screenFrame = screen.visibleFrame
            let x = screenFrame.midX - panelWidth / 2
            let y = screenFrame.midY - panelHeight / 2 + screenFrame.height * 0.1
            panel.setFrame(NSRect(x: x, y: y, width: panelWidth, height: panelHeight), display: true)
        }

        panel.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        logger.notice("show: panel ordered front, isVisible=\(panel.isVisible)")
    }

    func hide() {
        logger.notice("hide: ordering out panel")
        panel?.orderOut(nil)
    }

    private func selectAndPaste(asPlainText: Bool = false) {
        let entries = viewModel.filteredEntries
        let index = selectionState.selectedIndex
        guard index >= 0, index < entries.count else { return }
        let entry = entries[index]

        let _ = touch_entry(entry.id)
        let imageData = entry.isImage ? viewModel.loadImageData(for: entry.id) : nil
        PasteService.copyToClipboard(entry: entry, imageData: imageData, monitor: monitor, asPlainText: asPlainText)
        hide()

        if let app = previousApp {
            logger.notice("Activating previous app: \(app.localizedName ?? "unknown")")
            app.activate()
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(200))
                PasteService.simulatePaste()
            }
        } else {
            logger.warning("previousApp is nil, cannot paste")
        }
    }

    private func createPanel() {
        let panel = KeyablePanel(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 480),
            styleMask: [.borderless, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.titlebarAppearsTransparent = true
        panel.titleVisibility = .hidden
        panel.isMovableByWindowBackground = false
        panel.hidesOnDeactivate = true
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        let contentView = HistoryPanel(
            viewModel: viewModel,
            monitor: monitor,
            selectionState: selectionState
        ) { [weak self] entry in
            guard let self else { return }
            let _ = touch_entry(entry.id)
            let imageData = entry.isImage ? self.viewModel.loadImageData(for: entry.id) : nil
            PasteService.copyToClipboard(entry: entry, imageData: imageData, monitor: self.monitor)
            self.hide()
            if let app = self.previousApp {
                app.activate()
                Task { @MainActor in
                    try? await Task.sleep(for: .milliseconds(200))
                    PasteService.simulatePaste()
                }
            }
        }
        let hostingView = NSHostingView(rootView: contentView)
        hostingView.wantsLayer = true
        hostingView.layer?.backgroundColor = .clear
        hostingView.layer?.isOpaque = false
        hostingView.layer?.cornerRadius = 16
        hostingView.layer?.masksToBounds = true
        panel.contentView = hostingView

        panel.navigationKeyHandler = { [weak self] event in
            self?.handleKey(event) ?? false
        }

        self.panel = panel
    }

    private func handleKey(_ event: NSEvent) -> Bool {
        switch Int(event.keyCode) {
        case 126: // Up
            selectionState.moveUp()
            return true
        case 125: // Down
            selectionState.moveDown()
            return true
        case 36: // Return
            let asPlainText = event.modifierFlags.contains(.shift)
            selectAndPaste(asPlainText: asPlainText)
            return true
        case 53: // Escape
            hide()
            return true
        default:
            return false
        }
    }
}

class KeyablePanel: NSPanel {
    var navigationKeyHandler: ((NSEvent) -> Bool)?

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    override func sendEvent(_ event: NSEvent) {
        if event.type == .keyDown {
            let keyCode = Int(event.keyCode)
            // Intercept navigation keys BEFORE they reach the TextField
            if [126, 125, 36, 53].contains(keyCode) {
                if navigationKeyHandler?(event) == true {
                    return
                }
            }
        }

        super.sendEvent(event)
    }

    override func keyDown(with event: NSEvent) {
        // Silently consume any unhandled keys to prevent beep
    }
}
