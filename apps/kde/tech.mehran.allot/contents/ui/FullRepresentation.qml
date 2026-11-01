import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.extras as PlasmaExtras

import "../code/widget.mjs" as Allot

// The popup: a header with the summary and Keep Open, then the jobs grouped as Benchmark,
// Running and Waiting, the idle sessions, then Recently finished or the empty line.
PlasmaExtras.Representation {
    id: full

    property var snapshot: null
    property bool keepOpen: false
    property string iconDirectory: ""
    property var rowModel: null
    property var rowData: ({})
    property bool animate: false
    property var widget: null

    signal keepOpenToggled()

    readonly property bool hasSnapshot: full.snapshot !== null
    readonly property string glyph: full.hasSnapshot ? full.snapshot.header.glyph : "error"
    // The handoff's flyout: 360 wide, and as high as its content up to 640 or 70% of the screen.
    readonly property real maximumHeight: Math.min(Kirigami.Units.gridUnit * 36,
        Screen.desktopAvailableHeight * 0.7)

    Layout.preferredWidth: Kirigami.Units.gridUnit * 20
    Layout.minimumWidth: Kirigami.Units.gridUnit * 20
    Layout.preferredHeight: Math.min(full.header.implicitHeight + list.implicitHeight
        + Kirigami.Units.largeSpacing * 2, full.maximumHeight)
    Layout.maximumHeight: full.maximumHeight
    collapseMarginsHint: true

    header: PlasmaExtras.PlasmoidHeading {
        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing * 2

            Kirigami.Icon {
                Layout.leftMargin: Kirigami.Units.smallSpacing
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                source: full.glyph === "error" ? Allot.ERROR_ICON
                    : full.iconDirectory + Allot.MARK_ICON_FILE
                opacity: full.glyph === "idle" ? Allot.DIMMED : 1
            }

            PlasmaComponents.Label {
                Layout.fillWidth: true
                text: full.hasSnapshot ? full.snapshot.header.summary : ""
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Accessible.role: Accessible.StaticText
            }

            PlasmaComponents.ToolButton {
                icon.name: "window-pin"
                text: "Keep Open"
                display: PlasmaComponents.AbstractButton.IconOnly
                checkable: true
                checked: full.keepOpen
                onToggled: full.keepOpenToggled()
                PlasmaComponents.ToolTip.text: text
                PlasmaComponents.ToolTip.visible: hovered
                PlasmaComponents.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }
    }

    contentItem: PlasmaComponents.ScrollView {
        id: scroll

        PlasmaComponents.ScrollBar.horizontal.policy: PlasmaComponents.ScrollBar.AlwaysOff

        Column {
            id: list

            width: scroll.availableWidth
            opacity: full.hasSnapshot && full.snapshot.isLastKnown ? Allot.DIMMED : 1

            // A ListModel keeps each row's view while its key is shown, so a tick updates the
            // view in place and Kill? keeps its focus.
            Repeater {
                model: full.rowModel

                ListRow {
                    width: list.width
                    row: full.rowData[key]
                    animate: full.animate
                    widget: full.widget
                }
            }
        }
    }
}
