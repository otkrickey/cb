import AppKit
import SwiftUI

struct HistoryPanel: View {
    var viewModel: HistoryViewModel
    @ObservedObject var monitor: ClipboardMonitor
    var selectionState: SelectionState
    var onSelect: ((ClipboardEntryModel) -> Void)?

    @FocusState private var isSearchFocused: Bool

    var body: some View {
        GlassEffectContainer {
            VStack(spacing: 0) {
                searchBar
                Divider()

                HStack(spacing: 0) {
                    entryListPane
                        .frame(width: 280)

                    detailPane
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }

                Divider()
                bottomBar
            }
            .frame(width: 720, height: 480)
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .glassEffect(.regular, in: .rect(cornerRadius: 16))
        }
        .onAppear {
            isSearchFocused = true
        }
        .onChange(of: monitor.latestEntryTimestamp) {
            viewModel.loadEntries()
            selectionState.entryCount = viewModel.filteredEntries.count
        }
        .onChange(of: viewModel.searchText) {
            viewModel.performSearch()
            selectionState.selectedIndex = 0
            selectionState.entryCount = viewModel.filteredEntries.count
        }
        .onChange(of: viewModel.typeFilter) {
            selectionState.selectedIndex = 0
            selectionState.entryCount = viewModel.filteredEntries.count
        }
    }

    // MARK: - Search Bar

    private var searchBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)
            TextField("Filter", text: Bindable(viewModel).searchText)
                .textFieldStyle(.plain)
                .font(.body)
                .focused($isSearchFocused)
                .accessibilityLabel("Search clipboard history")
            Spacer()
            Label(viewModel.typeFilter.label, systemImage: viewModel.typeFilter.icon)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
                .glassEffect(.regular, in: .capsule)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    // MARK: - Entry List

    private var entryListPane: some View {
        VStack(spacing: 0) {
            if viewModel.filteredEntries.isEmpty {
                emptyState
            } else {
                entryList
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: "clipboard")
                .font(.system(size: 32))
                .foregroundStyle(.quaternary)
            Text("No items found")
                .font(.subheadline)
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var entryList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(viewModel.filteredEntries.enumerated()), id: \.element.id) { index, entry in
                        VStack(alignment: .leading, spacing: 0) {
                            if viewModel.shouldShowDateHeader(at: index) {
                                Text(viewModel.dateHeader(for: entry))
                                    .font(.subheadline.weight(.semibold))
                                    .foregroundStyle(.secondary)
                                    .padding(.horizontal, 12)
                                    .padding(.top, index == 0 ? 8 : 16)
                                    .padding(.bottom, 4)
                                    .accessibilityAddTraits(.isHeader)
                            }

                            ClipboardItemRow(
                                entry: entry,
                                isSelected: index == selectionState.selectedIndex,
                                thumbnail: entry.isImage ? viewModel.loadImage(for: entry.id) : nil,
                                loadImageData: { id in viewModel.loadImageData(for: id) }
                            )
                            .id(entry.id)
                            .onTapGesture {
                                onSelect?(entry)
                            }
                            .contextMenu {
                                Button("Delete", role: .destructive) {
                                    viewModel.deleteEntry(entry.id)
                                    selectionState.entryCount = viewModel.filteredEntries.count
                                }
                            }
                            .onAppear {
                                if index >= viewModel.filteredEntries.count - 5 {
                                    viewModel.loadMoreEntries()
                                }
                            }
                        }
                    }

                    if viewModel.isLoadingMore {
                        ProgressView()
                            .frame(maxWidth: .infinity)
                            .padding(8)
                    }
                }
                .padding(.horizontal, 6)
                .padding(.bottom, 6)
            }
            .scrollContentBackground(.hidden)
            .onChange(of: selectionState.selectedIndex) {
                if let entry = viewModel.filteredEntries[safe: selectionState.selectedIndex] {
                    withAnimation(.easeOut(duration: 0.12)) {
                        proxy.scrollTo(entry.id, anchor: .center)
                    }
                }
            }
        }
    }

    // MARK: - Detail Pane

    @ViewBuilder
    private var detailPane: some View {
        if let entry = viewModel.filteredEntries[safe: selectionState.selectedIndex] {
            VStack(alignment: .leading, spacing: 0) {
                contentPreview(for: entry)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)

                Divider()
                    .padding(.horizontal, 16)

                informationSection(for: entry)
            }
        } else {
            VStack(spacing: 10) {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.system(size: 40))
                    .foregroundStyle(.quaternary)
                Text("Select an item to preview")
                    .font(.subheadline)
                    .foregroundStyle(.tertiary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    @ViewBuilder
    private func contentPreview(for entry: ClipboardEntryModel) -> some View {
        if let text = entry.textContent {
            SelectableTextView(text: text)
        } else if entry.isImage {
            if let nsImage = viewModel.loadImage(for: entry.id) {
                Image(nsImage: nsImage)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(16)
            } else {
                VStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.large)
                    Text("Loading image...")
                        .font(.subheadline)
                        .foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func informationSection(for entry: ClipboardEntryModel) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Information")
                .font(.headline)
                .foregroundStyle(.primary)
                .padding(.horizontal, 16)
                .padding(.top, 12)
                .padding(.bottom, 8)

            VStack(spacing: 0) {
                if let app = entry.sourceApp, !app.isEmpty {
                    infoRow(label: "Source", value: app)
                    Divider().padding(.leading, 16)
                }
                infoRow(label: "Content type", value: entry.displayType)
                if let text = entry.textContent {
                    Divider().padding(.leading, 16)
                    infoRow(label: "Characters", value: "\(text.count)")
                    Divider().padding(.leading, 16)
                    infoRow(label: "Words", value: "\(entry.wordCount)")
                }
                if entry.isImage, let nsImage = viewModel.loadImage(for: entry.id) {
                    Divider().padding(.leading, 16)
                    infoRow(label: "Size", value: "\(Int(nsImage.size.width)) x \(Int(nsImage.size.height))")
                }
                Divider().padding(.leading, 16)
                infoRow(label: "Times copied", value: "\(entry.copyCount)")
                Divider().padding(.leading, 16)
                infoRow(label: "Last copied", value: entry.formattedDate)
                if entry.copyCount > 1 {
                    Divider().padding(.leading, 16)
                    infoRow(label: "First copied", value: entry.formattedFirstCopied)
                }
            }
            .padding(.bottom, 12)
        }
    }

    private func infoRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline)
                .foregroundStyle(.primary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
    }

    // MARK: - Bottom Bar

    private var bottomBar: some View {
        HStack(spacing: 0) {
            Label("Clipboard History", systemImage: "clipboard.fill")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .labelStyle(.titleAndIcon)

            Spacer()

            HStack(spacing: 12) {
                if !viewModel.targetAppName.isEmpty {
                    HStack(spacing: 6) {
                        Text("Paste to \(viewModel.targetAppName)")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                        HStack(spacing: 8) {
                            Text("⇧⏎")
                                .font(.subheadline.weight(.medium))
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .glassEffect(.regular, in: .rect(cornerRadius: 4))
                            Text("Plain")
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                            Text("⏎")
                                .font(.subheadline.weight(.medium))
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .glassEffect(.regular, in: .rect(cornerRadius: 4))
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }
}

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

// MARK: - Selectable Text View

struct SelectableTextView: NSViewRepresentable {
    let text: String

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let textView = scrollView.documentView as! NSTextView
        textView.isEditable = false
        textView.isSelectable = true
        textView.backgroundColor = .clear
        textView.drawsBackground = false
        textView.font = .systemFont(ofSize: NSFont.systemFontSize)
        textView.textColor = .labelColor
        textView.textContainerInset = NSSize(width: 12, height: 12)
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.drawsBackground = false
        scrollView.backgroundColor = .clear
        scrollView.borderType = .noBorder
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        let textView = scrollView.documentView as! NSTextView
        if textView.string != text {
            textView.string = text
            textView.scrollToBeginningOfDocument(nil)
        }
    }
}
