import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/widget.mjs" as Allot

// One running or waiting job: its session line, then its command in the fixed-width font.
ColumnLayout {
    id: row

    required property var modelData

    spacing: 0

    PlasmaComponents.Label {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing * 1.5
        text: Allot.stripText(row.modelData.session)
        font: Kirigami.Theme.smallFont
        elide: Text.ElideRight
        PlasmaComponents.ToolTip.text: row.modelData.session.projectPath
        PlasmaComponents.ToolTip.visible: hover.hovered && row.modelData.session.projectPath !== ""

        HoverHandler {
            id: hover
        }
    }

    PlasmaComponents.Label {
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing * 1.5
        text: row.modelData.command
        font: Kirigami.Theme.fixedWidthFont
        elide: Text.ElideRight
    }
}
