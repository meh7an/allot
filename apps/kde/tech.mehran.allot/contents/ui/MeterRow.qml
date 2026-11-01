import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

import "../code/card-view.mjs" as CardView

// One bullet-bar row: label, bar and value, with the detail in a tooltip. A screen reader gets the
// whole row as one line, since its tab order has no stop here and the tooltip needs a pointer.
RowLayout {
    id: meter

    required property var row
    required property bool animate
    required property real labelWidth
    required property real valueWidth

    Layout.preferredHeight: Math.max(Kirigami.Units.largeSpacing * 3.5,
        value.implicitHeight + Kirigami.Units.smallSpacing)
    spacing: Kirigami.Units.largeSpacing
    Accessible.role: Accessible.StaticText
    Accessible.name: CardView.meterName(row)
    Accessible.description: row.detail

    PlasmaComponents.ToolTip.text: row.detail
    PlasmaComponents.ToolTip.visible: hover.hovered && row.detail !== ""
    PlasmaComponents.ToolTip.delay: Kirigami.Units.toolTipDelay

    HoverHandler {
        id: hover
    }

    PlasmaComponents.Label {
        Layout.preferredWidth: meter.labelWidth
        text: meter.row.label
        font.pointSize: Kirigami.Theme.smallFont.pointSize
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        Accessible.ignored: true
    }

    BulletBar {
        Layout.fillWidth: true
        row: meter.row
        animate: meter.animate
    }

    RowLayout {
        // A label inside that fills would otherwise make this fill too, and squeeze the bar.
        Layout.fillWidth: false
        Layout.preferredWidth: meter.valueWidth
        spacing: Kirigami.Units.smallSpacing

        Sparkline {
            visible: meter.row.spark.length > 0
            samples: meter.row.spark
        }

        PlasmaComponents.Label {
            id: value

            Layout.fillWidth: true
            horizontalAlignment: Text.AlignRight
            text: meter.row.value
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.features: { "tnum": 1 }
            elide: Text.ElideLeft
            Accessible.ignored: true
        }
    }
}
