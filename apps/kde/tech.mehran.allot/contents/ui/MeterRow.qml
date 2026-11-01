import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

// One bullet-bar row: label, bar and value, with the detail in a tooltip.
RowLayout {
    id: meter

    required property var row
    required property bool animate

    Layout.preferredHeight: Kirigami.Units.largeSpacing * 3.5
    spacing: Kirigami.Units.largeSpacing

    PlasmaComponents.ToolTip.text: row.detail
    PlasmaComponents.ToolTip.visible: hover.hovered && row.detail !== ""
    PlasmaComponents.ToolTip.delay: Kirigami.Units.toolTipDelay

    HoverHandler {
        id: hover
    }

    PlasmaComponents.Label {
        Layout.preferredWidth: Kirigami.Units.largeSpacing * 6
        text: meter.row.label
        font.pointSize: Kirigami.Theme.smallFont.pointSize
        font.weight: Font.DemiBold
        elide: Text.ElideRight
    }

    BulletBar {
        Layout.fillWidth: true
        row: meter.row
        animate: meter.animate
    }

    RowLayout {
        // A label inside that fills would otherwise make this fill too, and squeeze the bar.
        Layout.fillWidth: false
        Layout.preferredWidth: Kirigami.Units.largeSpacing * 13.5
        spacing: Kirigami.Units.smallSpacing

        Sparkline {
            visible: meter.row.spark.length > 0
            samples: meter.row.spark
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignRight
            text: meter.row.value
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.features: { "tnum": 1 }
            elide: Text.ElideLeft
        }
    }
}
