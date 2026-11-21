import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// What goes under a setting's help: its notes, then its issue with an error icon, so an error is
// never told by colour alone (DESIGN.md section 11).
ColumnLayout {
    id: notes

    property var view: null

    Layout.fillWidth: true
    spacing: 0
    visible: notes.view !== null && (notes.view.notes.length > 0 || notes.view.issue !== null)

    Repeater {
        model: notes.view === null ? [] : notes.view.notes

        QQC2.Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            wrapMode: Text.Wrap
            font: Kirigami.Theme.smallFont
        }
    }

    RowLayout {
        Layout.fillWidth: true
        visible: notes.view !== null && notes.view.issue !== null

        Kirigami.Icon {
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            source: "dialog-error"
            Accessible.name: "Error"
        }

        // Also in the field's description while it shows, so Orca reads it with the field.
        QQC2.Label {
            Layout.fillWidth: true
            text: notes.view === null ? "" : (notes.view.issue ?? "")
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
        }
    }
}
