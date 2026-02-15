import AppKit
import os

private let logger = Logger(subsystem: "com.otkrickey.cb", category: "HistoryViewModel")

private struct FFIResponse<T: Decodable>: Decodable {
    let ok: T?
    let error: String?
}

enum ContentTypeFilter: CaseIterable {
    case all, plainText, image, filePath

    var label: String {
        switch self {
        case .all: return "All Types"
        case .plainText: return "Text"
        case .image: return "Images"
        case .filePath: return "Files"
        }
    }

    var icon: String {
        switch self {
        case .all: return "square.stack"
        case .plainText: return "doc.text.fill"
        case .image: return "photo"
        case .filePath: return "folder.fill"
        }
    }

    var rawContentType: String {
        switch self {
        case .all: return ""
        case .plainText: return "PlainText"
        case .image: return "Image"
        case .filePath: return "FilePath"
        }
    }

    var next: ContentTypeFilter {
        let cases = ContentTypeFilter.allCases
        let idx = cases.firstIndex(of: self)!
        return cases[(idx + 1) % cases.count]
    }
}

struct ClipboardEntryModel: Identifiable, Codable {
    let id: Int64
    let content_type: String
    let text_content: String?
    let source_app: String?
    let created_at: Int64
    let copy_count: Int64
    let first_copied_at: Int64

    var contentType: String { content_type }
    var textContent: String? { text_content }
    var sourceApp: String? { source_app }
    var createdAt: Date { Date(timeIntervalSince1970: TimeInterval(created_at) / 1000.0) }
    var firstCopiedAt: Date { Date(timeIntervalSince1970: TimeInterval(first_copied_at) / 1000.0) }
    var copyCount: Int64 { copy_count }

    var isImage: Bool { content_type == "Image" }
    var isEmpty: Bool { id == -1 }

    static let empty = ClipboardEntryModel(id: -1, content_type: "", text_content: nil, source_app: nil, created_at: 0, copy_count: 1, first_copied_at: 0)

    var previewText: String {
        if let text = text_content {
            return String(text.prefix(200))
        }
        if isImage {
            return "[Image]"
        }
        return ""
    }

    var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        return formatter.localizedString(for: createdAt, relativeTo: Date())
    }

    var displayType: String {
        switch content_type {
        case "Image": return "Image"
        case "FilePath": return "File Path"
        case "RichText": return "Rich Text"
        default: return "Plain Text"
        }
    }

    var characterCount: Int {
        text_content?.count ?? 0
    }

    var wordCount: Int {
        guard let text = text_content else { return 0 }
        var count = 0
        text.enumerateSubstrings(in: text.startIndex..., options: [.byWords, .substringNotRequired]) { _, _, _, _ in
            count += 1
        }
        return count
    }

    var lineCount: Int {
        text_content?.components(separatedBy: .newlines).count ?? 0
    }

    var formattedDate: String {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .medium
        return formatter.string(from: createdAt)
    }

    var formattedFirstCopied: String {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .medium
        return formatter.string(from: firstCopiedAt)
    }
}

@MainActor
@Observable
class HistoryViewModel {
    var entries: [ClipboardEntryModel] = []
    var searchText: String = ""
    var targetAppName: String = ""
    var typeFilter: ContentTypeFilter = .all
    @ObservationIgnored var imageCache: NSCache<NSNumber, NSImage> = {
        let cache = NSCache<NSNumber, NSImage>()
        cache.countLimit = 100
        cache.totalCostLimit = 50 * 1024 * 1024
        return cache
    }()
    @ObservationIgnored private var loadingImageIds: Set<Int64> = []
    var searchResults: [ClipboardEntryModel] = []
    @ObservationIgnored private var searchTask: Task<Void, Never>?
    var hasMore: Bool = true
    var isLoadingMore: Bool = false

    func loadImage(for id: Int64) -> NSImage? {
        let key = NSNumber(value: id)
        if let cached = imageCache.object(forKey: key) { return cached }

        // Avoid duplicate loads
        guard !loadingImageIds.contains(id) else { return nil }
        loadingImageIds.insert(id)

        Task { @MainActor [weak self] in
            guard let self else { return }
            guard let rustVec = get_entry_image(id) else {
                self.loadingImageIds.remove(id)
                return
            }
            let data = Data(bytes: rustVec.as_ptr(), count: rustVec.len())
            guard let image = NSImage(data: data) else {
                self.loadingImageIds.remove(id)
                return
            }
            self.imageCache.setObject(image, forKey: key, cost: data.count)
            self.loadingImageIds.remove(id)
            // Trigger observation update
            self.filteredEntries = self.filteredEntries
        }
        return nil
    }

    func loadImageData(for id: Int64) -> Data? {
        guard let rustVec = get_entry_image(id) else { return nil }
        return Data(bytes: rustVec.as_ptr(), count: rustVec.len())
    }

    func performSearch() {
        searchTask?.cancel()
        searchTask = Task {
            try? await Task.sleep(for: .seconds(0.3))
            guard !Task.isCancelled else { return }
            let query = searchText.trimmingCharacters(in: .whitespaces)
            if query.isEmpty {
                searchResults = []
                updateFilteredEntries()
                return
            }
            let decoded = await Task.detached {
                let json = search_entries(query, Int32(50))
                let jsonString = json.toString()
                guard let data = jsonString.data(using: .utf8) else { return [ClipboardEntryModel]() }
                if let response = try? JSONDecoder().decode(FFIResponse<[ClipboardEntryModel]>.self, from: data),
                   let entries = response.ok {
                    return entries
                }
                return [ClipboardEntryModel]()
            }.value
            guard !Task.isCancelled else { return }
            searchResults = decoded
            updateFilteredEntries()
        }
    }

    private(set) var filteredEntries: [ClipboardEntryModel] = []

    private func updateFilteredEntries() {
        let base = searchText.trimmingCharacters(in: .whitespaces).isEmpty ? entries : searchResults
        if typeFilter == .all {
            filteredEntries = base
        } else {
            filteredEntries = base.filter { $0.contentType == typeFilter.rawContentType }
        }
    }

    func cycleTypeFilter() {
        typeFilter = typeFilter.next
        updateFilteredEntries()
    }

    func shouldShowDateHeader(at index: Int) -> Bool {
        let list = filteredEntries
        guard index < list.count else { return false }
        if index == 0 { return true }
        let current = Calendar.current.startOfDay(for: list[index].createdAt)
        let previous = Calendar.current.startOfDay(for: list[index - 1].createdAt)
        return current != previous
    }

    func dateHeader(for entry: ClipboardEntryModel) -> String {
        let calendar = Calendar.current
        let date = entry.createdAt
        if calendar.isDateInToday(date) { return "Today" }
        if calendar.isDateInYesterday(date) { return "Yesterday" }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .none
        return formatter.string(from: date)
    }

    func loadEntries() {
        let json = get_recent_entries(50)
        let jsonString = json.toString()
        guard let data = jsonString.data(using: .utf8) else { return }
        if let response = try? JSONDecoder().decode(FFIResponse<[ClipboardEntryModel]>.self, from: data) {
            if let entries = response.ok {
                self.entries = entries
            } else if let error = response.error {
                logger.error("loadEntries failed: \(error)")
            }
        }
        updateFilteredEntries()
    }

    func loadMoreEntries() {
        guard hasMore, !isLoadingMore else { return }
        guard let lastTimestamp = entries.last?.created_at else { return }
        isLoadingMore = true

        let json = get_entries_before(lastTimestamp, Int32(50))
        let jsonString = json.toString()
        guard let data = jsonString.data(using: .utf8) else {
            isLoadingMore = false
            return
        }
        if let response = try? JSONDecoder().decode(FFIResponse<[ClipboardEntryModel]>.self, from: data),
           let newEntries = response.ok, !newEntries.isEmpty {
            entries.append(contentsOf: newEntries)
        } else {
            hasMore = false
        }
        isLoadingMore = false
        updateFilteredEntries()
    }

    func deleteEntry(_ id: Int64) {
        let deleted = delete_entry(id)
        if deleted {
            entries.removeAll { $0.id == id }
            updateFilteredEntries()
        }
    }
}
