import SwiftUI

struct ClipboardItemRow: View {
    let entry: ClipboardEntryModel
    var isSelected: Bool = false
    var thumbnail: NSImage? = nil
    var loadImageData: ((Int64) -> Data?)? = nil

    var body: some View {
        HStack(spacing: 10) {
            if entry.isImage, let thumbnail {
                Image(nsImage: thumbnail)
                    .resizable()
                    .aspectRatio(contentMode: .fill)
                    .frame(width: 20, height: 20)
                    .clipShape(RoundedRectangle(cornerRadius: 4))
                    .drawingGroup()
            } else {
                Image(systemName: iconName)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .frame(width: 20, height: 20)
            }

            Text(entry.previewText)
                .font(.body)
                .lineLimit(1)
                .foregroundStyle(.primary)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .contentShape(Rectangle())
        .background {
            if isSelected {
                RoundedRectangle(cornerRadius: 6)
                    .fill(.selection)
            }
        }
        .animation(.easeOut(duration: 0.1), value: isSelected)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(entry.displayType): \(entry.previewText)")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
        .onDrag {
            if entry.isImage {
                if let imageData = loadImageData?(entry.id), let nsImage = NSImage(data: imageData) {
                    return NSItemProvider(object: nsImage)
                } else {
                    return NSItemProvider()
                }
            } else if let text = entry.textContent {
                return NSItemProvider(object: text as NSString)
            } else {
                return NSItemProvider()
            }
        }
    }

    private var iconName: String {
        switch entry.contentType {
        case "Image": return "photo"
        case "FilePath": return "folder.fill"
        case "RichText": return "textformat"
        default: return "doc.text.fill"
        }
    }
}
