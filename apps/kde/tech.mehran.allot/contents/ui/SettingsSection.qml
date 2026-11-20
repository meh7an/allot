import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// One group of a Settings page: a separator above it, its title and help, then its rows.
ColumnLayout {
    id: section

    property string title: ""
    property string help: ""
    default property alias rows: body.data

    Layout.fillWidth: true
    Layout.leftMargin: Kirigami.Units.largeSpacing
    Layout.rightMargin: Kirigami.Units.largeSpacing
    spacing: Kirigami.Units.smallSpacing

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    Kirigami.Heading {
        Layout.fillWidth: true
        level: 3
        text: section.title
        visible: section.title !== ""
        wrapMode: Text.Wrap
    }

    HelpLabel {
        text: View.escaped(section.help)
        visible: section.help !== ""
    }

    ColumnLayout {
        id: body

        Layout.fillWidth: true
        spacing: Kirigami.Units.largeSpacing
    }
}
