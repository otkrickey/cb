import Foundation

@MainActor
@Observable
class SelectionState {
    var selectedIndex: Int = 0
    var entryCount: Int = 0

    func moveUp() {
        if selectedIndex > 0 {
            selectedIndex -= 1
        }
    }

    func moveDown() {
        if selectedIndex < entryCount - 1 {
            selectedIndex += 1
        }
    }
}
