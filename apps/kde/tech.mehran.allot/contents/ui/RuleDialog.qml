import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// The dialog of one rule: program, arguments as chips, class, and its own Test a command box,
// which tests the draft before Save. Save writes the whole list; Cancel and Escape drop it.
Kirigami.Dialog {
    id: dialog

    property var host: null
    property var rules: null
    property var draft: null
    // The list whose rule opened the dialog, and the rule's row (-1 for a new one).
    property var opener: null
    property int openerRow: -1
    readonly property var program: {
        dialog.host.revision;
        return dialog.draft === null ? null : View.programView(dialog.draft, dialog.host.number);
    }
    readonly property var options: dialog.draft === null ? [] : dialog.draft.classOptions

    function edit(rules, draft, opener, row) {
        dialog.rules = rules;
        dialog.draft = draft;
        dialog.opener = opener;
        dialog.openerRow = row;
        dialog.title = View.ruleDialogTitle(draft);
        programField.text = draft.program;
        classBox.currentIndex = dialog.options.findIndex((option) =>
            option.value === draft.ruleClass);
        tester.clear();
        draft.onChange = () => {
            dialog.host.changed();
            tester.run();
        };
        dialog.open();
    }

    function save() {
        if (dialog.rules.save(dialog.draft)) {
            dialog.close();
        } else {
            dialog.host.changed();
        }
    }

    // Text scaling: the width follows the font's height, which `gridUnit` does not.
    preferredWidth: metrics.height * 28
    standardButtons: Kirigami.Dialog.NoButton
    customFooterActions: [
        Kirigami.Action {
            text: "Save"
            icon.name: "document-save"
            onTriggered: dialog.save()
        },
        Kirigami.Action {
            text: "Cancel"
            icon.name: "dialog-cancel"
            onTriggered: dialog.close()
        }
    ]
    onOpened: programField.forceActiveFocus()
    onClosed: {
        if (dialog.draft !== null) {
            dialog.draft.onChange = null;
        }
        dialog.draft = null;
        dialog.host.changed();
        dialog.opener?.focusRow(dialog.openerRow);
    }

    FontMetrics {
        id: metrics

        font: Kirigami.Theme.defaultFont
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            QQC2.Label {
                Layout.fillWidth: true
                text: dialog.program === null ? "" : dialog.program.label
                wrapMode: Text.Wrap
                Accessible.ignored: true
            }

            QQC2.TextField {
                id: programField

                Layout.fillWidth: true
                font.family: "monospace"
                Accessible.name: dialog.program === null ? "" : dialog.program.label
                Accessible.description: dialog.program === null ? "" : dialog.program.description
                onTextEdited: dialog.draft.setProgram(programField.text)
            }

            HelpLabel {
                text: dialog.program === null ? "" : View.escaped(dialog.program.help)
            }

            RowNotes {
                view: dialog.program === null ? null
                    : ({ notes: [], issue: dialog.program.issue })
            }
        }

        Loader {
            Layout.fillWidth: true
            active: dialog.draft !== null
            sourceComponent: SettingChips {
                host: dialog.host
                setting: dialog.draft.args
                inDialog: true
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            QQC2.Label {
                Layout.fillWidth: true
                text: dialog.draft === null ? "" : dialog.draft.classSpec.label
                wrapMode: Text.Wrap
                Accessible.ignored: true
            }

            QQC2.ComboBox {
                id: classBox

                Layout.fillWidth: true
                model: dialog.options.map((option) => option.label)
                Accessible.name: dialog.draft === null ? "" : dialog.draft.classSpec.label
                Accessible.description: dialog.draft === null ? "" : dialog.draft.classSpec.help
                onActivated: (index) => {
                    const option = dialog.options[index];
                    if (option !== undefined && option.value !== dialog.draft.ruleClass) {
                        dialog.draft.setClass(option.value);
                    }
                }
            }

            HelpLabel {
                text: dialog.draft === null ? "" : View.escaped(dialog.draft.classSpec.help)
            }
        }

        SettingsSection {
            Layout.leftMargin: 0
            Layout.rightMargin: 0
            title: View.TEST_COMMAND_TITLE
            help: View.TEST_RULE_HELP

            CommandTest {
                id: tester

                host: dialog.host
                rules: dialog.rules
                draft: dialog.draft
            }
        }
    }
}
