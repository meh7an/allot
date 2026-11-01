import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents

// One entry of the Resources card's legend: a swatch, plain or hatched, and its name.
RowLayout {
    id: entry

    required property string text
    property color swatchColor: "transparent"
    property int swatchWidth: 10
    property bool hatched: false

    spacing: Kirigami.Units.smallSpacing

    Rectangle {
        visible: !entry.hatched
        implicitWidth: entry.swatchWidth
        implicitHeight: 6
        radius: entry.swatchWidth > 2 ? 2 : 0
        color: entry.swatchColor
    }

    HatchedTrack {
        visible: entry.hatched
        implicitWidth: 14
        implicitHeight: 6
        radius: 2
        headroom: 0
    }

    PlasmaComponents.Label {
        text: entry.text
        font: Kirigami.Theme.smallFont
        opacity: 0.8
    }
}
