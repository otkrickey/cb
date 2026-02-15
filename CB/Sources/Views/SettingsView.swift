import SwiftUI

struct SettingsView: View {
    @AppStorage("retentionDays") private var retentionDays: Int = 7

    var body: some View {
        Form {
            Section("データ保持") {
                Picker("保持期間", selection: $retentionDays) {
                    ForEach(1...7, id: \.self) { days in
                        Text("\(days)日間").tag(days)
                    }
                }
            }

            Section("ショートカット") {
                ShortcutRecorderView()
                LabeledContent("ペースト", value: "⏎")
                LabeledContent("プレーンテキストペースト", value: "⇧⏎")
            }

            Section("情報") {
                LabeledContent("バージョン") {
                    Text(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0.0")
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 400, height: 300)
    }
}
