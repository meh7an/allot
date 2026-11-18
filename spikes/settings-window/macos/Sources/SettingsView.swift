import SwiftUI

/// Two sample pages in a sidebar, the shape M0.4 may pick; the text field takes focus on open so
/// the checklist can tell whether typing lands in the window.
struct SettingsView: View {
    enum Page: String, CaseIterable, Identifiable {
        case general = "General"
        case admission = "Admission"

        var id: Self { self }
    }

    let diagnostics: WindowDiagnostics

    @State private var page: Page? = .general
    @State private var example = true
    @State private var headroom = 15.0
    @State private var probe = ""
    @FocusState private var probeFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            NavigationSplitView {
                List(Page.allCases, selection: $page) { page in
                    Text(page.rawValue)
                }
                .navigationSplitViewColumnWidth(min: 160, ideal: 180)
            } detail: {
                Form {
                    switch page ?? .general {
                    case .general:
                        TextField("Type here to check keyboard focus", text: $probe)
                            .focused($probeFocused)
                        Toggle("Example switch", isOn: $example)
                        Stepper("Headroom \(Int(headroom)) %", value: $headroom, in: 0...90)
                        Slider(value: $headroom, in: 0...90) {
                            Text("Same value as a slider")
                        }
                    case .admission:
                        LabeledContent("cpu_heavy", value: "8 cores, min 4")
                        LabeledContent("cpu_medium", value: "4 cores, min 2")
                        LabeledContent("gpu_heavy", value: "2 cores, min 1")
                    }
                }
                .formStyle(.grouped)
            }

            Divider()
            Text(diagnostics.text)
                .font(.system(size: 11, design: .monospaced))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
        }
        .onAppear {
            probeFocused = true
        }
    }
}
