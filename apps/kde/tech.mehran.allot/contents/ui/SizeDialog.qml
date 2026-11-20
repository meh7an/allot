import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// The edit dialog of a class or preset: Save writes the whole row at once (DESIGN.md D2, D5);
// Cancel and Escape drop the changes. The line under the sizes says how many fit with them.
Kirigami.Dialog {
    id: dialog

    property var host: null
    property var table: null
    property var draft: null
    property string fitText: ""
    property var preview: null
    readonly property var rename: {
        dialog.host.revision;
        return dialog.draft === null ? null : View.nameView(dialog.draft, dialog.host.number);
    }
    readonly property string live: {
        dialog.host.revision;
        if (dialog.draft === null) {
            return "";
        }
        return View.renameWarning(dialog.draft) ?? dialog.fitText;
    }

    function edit(table, draft) {
        dialog.table = table;
        dialog.draft = draft;
        dialog.fitText = View.NOT_YET_TEXT;
        dialog.title = View.sizeDialogTitle(table, draft);
        dialog.preview = View.debounced(dialog.host.timers, () =>
            View.sizeDialogFit(dialog.host.model.session, table, draft, dialog.host.number)
                .then((text) => {
                    if (text !== null && dialog.draft === draft) {
                        dialog.fitText = text;
                    }
                }), (error) => console.warn(`Allot: ${error}`));
        draft.onChange = () => {
            dialog.host.changed();
            dialog.preview.run();
        };
        nameField.text = draft.name;
        dialog.preview.run();
        dialog.open();
    }

    function save() {
        if (dialog.table.save(dialog.draft)) {
            dialog.close();
        } else {
            dialog.host.changed();
        }
    }

    preferredWidth: Kirigami.Units.gridUnit * 28
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
    // DESIGN.md section 11: a dialog takes focus on its first field.
    onOpened: {
        const first = dialog.rename !== null ? nameField : fields.nextItemInFocusChain(true);
        first?.forceActiveFocus();
    }
    onClosed: {
        if (dialog.draft !== null) {
            dialog.draft.onChange = null;
        }
        dialog.preview?.cancel();
        dialog.draft = null;
        dialog.host.changed();
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        ColumnLayout {
            Layout.fillWidth: true
            visible: dialog.rename !== null
            spacing: 0

            QQC2.Label {
                text: dialog.rename === null ? "" : dialog.rename.label
            }

            QQC2.TextField {
                id: nameField

                Layout.fillWidth: true
                Accessible.name: dialog.rename === null ? "" : dialog.rename.label
                onTextEdited: dialog.draft.setName(nameField.text)
            }

            HelpLabel {
                text: dialog.rename === null ? "" : View.escaped(dialog.rename.help)
            }

            RowNotes {
                view: dialog.rename === null ? null : ({ notes: [], issue: dialog.rename.issue })
            }
        }

        ColumnLayout {
            id: fields

            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Repeater {
                // By index, so each row gets the draft's own model, not a plain copy of it.
                model: dialog.draft === null ? 0 : dialog.draft.fields.length

                ColumnLayout {
                    id: field

                    required property int index
                    readonly property var fieldModel: dialog.draft.fields[index]
                    readonly property bool minimum: field.fieldModel.key === "min_cores"
                    readonly property var automatic: {
                        dialog.host.revision;
                        return field.minimum
                            ? View.minimumView(dialog.draft, field.fieldModel, dialog.host.number)
                            : null;
                    }

                    Layout.fillWidth: true
                    spacing: 0

                    QQC2.CheckBox {
                        id: automaticBox

                        visible: field.minimum
                        text: "Automatic minimum"
                        checked: field.automatic !== null && field.automatic.automatic
                        onToggled: field.fieldModel.setAutomatic(checked,
                            field.automatic.automaticValue)
                    }

                    HelpLabel {
                        Layout.leftMargin: automaticBox.indicator.width + automaticBox.spacing
                        visible: field.minimum
                        text: field.automatic === null ? "" : View.escaped(field.automatic.help)
                    }

                    SettingNumber {
                        visible: !field.minimum || !field.automatic.automatic
                        host: dialog.host
                        setting: field.fieldModel
                        inDialog: true
                    }
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: dialog.live
            visible: text !== ""
            wrapMode: Text.Wrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        QQC2.Button {
            readonly property var resettable: {
                dialog.host.revision;
                return dialog.draft === null ? null
                    : View.resettableRow(dialog.table, dialog.draft);
            }

            visible: resettable !== null
            text: "Reset to Default"
            onClicked: {
                const removed = View.removeSizeRow(dialog.table, resettable);
                dialog.host.notice(removed.text, removed.undo);
                dialog.close();
            }
        }
    }
}
