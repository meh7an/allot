import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// Test a command (DESIGN.md D6): a command line, tested in Bash once the user pauses, against
// the rules on the page or a rule dialog's draft. `tested` hands on allotd's answer, so the rule
// list can mark the rules it matched.
ColumnLayout {
    id: box

    property var host: null
    // The rules on the page, or with `draft` a rule dialog's draft of them.
    property var rules: null
    property var draft: null
    property var lines: []
    property var tester: null

    signal tested(var test)

    function run() {
        box.tester?.run();
    }

    function clear() {
        field.text = "";
        box.lines = [];
    }

    Layout.fillWidth: true
    spacing: Kirigami.Units.smallSpacing

    Component.onCompleted: {
        box.tester = View.debounced(box.host.timers, () =>
            View.testCommand(box.host.model.session, field.text,
                View.testAgainst(box.rules, box.draft), box.host.number)
                .then((outcome) => {
                    if (outcome !== null) {
                        // The result is the outcome of the user's typing, so it is spoken
                        // (M7.1), once per new result.
                        const spoken = outcome.lines.join(" ");
                        if (spoken !== "" && spoken !== box.lines.join(" ")) {
                            box.host.announce(spoken);
                        }
                        box.lines = outcome.lines;
                        box.tested(outcome.test);
                    }
                }), (error) => console.warn(`Allot: ${error}`));
    }
    Component.onDestruction: box.tester?.cancel()

    QQC2.TextField {
        id: field

        Layout.fillWidth: true
        placeholderText: "Command line"
        font.family: "monospace"
        Accessible.name: "Command line"
        Accessible.description: box.draft === null ? View.TEST_COMMAND_HELP : View.TEST_RULE_HELP
        onTextChanged: box.run()
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: box.lines.join("\n")
        visible: box.lines.length > 0
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
    }
}
