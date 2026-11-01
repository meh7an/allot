import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

// A group heading: the state icon and, for example, `Running · 2`.
RowLayout {
    id: heading

    required property string icon
    required property color iconColor
    required property string text

    spacing: Kirigami.Units.smallSpacing * 1.5

    Kirigami.Icon {
        implicitWidth: Kirigami.Units.iconSizes.small
        implicitHeight: Kirigami.Units.iconSizes.small
        source: heading.icon
        isMask: true
        color: heading.iconColor
    }

    PlasmaComponents.Label {
        Layout.fillWidth: true
        text: heading.text
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        opacity: 0.8
    }
}
