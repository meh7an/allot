import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.extras as PlasmaExtras

import "../code/widget.mjs" as Allot

// The popup: a header with the summary, Keep Open and More options; then the service banner and
// the Stop question; the jobs grouped as Benchmark, Running and Waiting, the idle sessions, the
// first-run card, then Recently finished or the empty line; then the docked Resources card.
PlasmaExtras.Representation {
    id: full

    property var snapshot: null
    property var resources: null
    property var banner: null
    property var settings: ({ meters: {}, notifications: {} })
    property string stopQuestion: ""
    property bool doNotDisturb: false
    property bool notificationsOff: false
    property string meterCount: ""
    property string notificationCount: ""
    property bool keepOpen: false
    property string iconDirectory: ""
    property var rowModel: null
    property var rowData: ({})
    property bool animate: false
    property var widget: null
    // A new object for every Show, so showing the same row twice scrolls twice.
    property var scrollRequest: null

    signal keepOpenToggled()

    readonly property bool hasSnapshot: full.snapshot !== null
    readonly property string glyph: full.hasSnapshot ? full.snapshot.header.glyph : "error"
    // The handoff's flyout: 360 wide, and as high as its content up to 640 or 70% of the screen.
    readonly property real maximumHeight: Math.min(Kirigami.Units.gridUnit * 36,
        Screen.desktopAvailableHeight * 0.7)
    readonly property real bottomGap: Kirigami.Units.smallSpacing * 2
    readonly property real blocksHeight: stopCard.implicitHeight + serviceBanner.implicitHeight
        + resourcesCard.implicitHeight
    readonly property real wantedHeight: Math.min(full.header.implicitHeight + full.blocksHeight
        + list.implicitHeight + full.bottomGap, full.maximumHeight)
    // From the height the popup got, not the one asked for, which it reaches a frame later.
    readonly property real listHeight: Math.max(0, Math.min(list.implicitHeight,
        full.contentItem.height - full.blocksHeight - full.bottomGap))

    function showRequestedRow() {
        const flickable = scroll.contentItem;
        const key = full.scrollRequest.key;
        let top = 0;
        for (let index = 0; index < rows.count; index += 1) {
            const row = rows.itemAt(index);
            if (row.key === key) {
                top = row.y;
            }
        }
        flickable.contentY = Math.max(0, Math.min(top, flickable.contentHeight - flickable.height));
    }

    Layout.preferredWidth: Kirigami.Units.gridUnit * 20
    Layout.minimumWidth: Kirigami.Units.gridUnit * 20
    // Plasma saves a popup's last size and, once it has one, no longer follows the preferred
    // height. It still obeys the minimum and maximum, so the popup stays as high as its content.
    Layout.preferredHeight: full.wantedHeight
    Layout.minimumHeight: full.wantedHeight
    Layout.maximumHeight: full.wantedHeight
    collapseMarginsHint: true

    onScrollRequestChanged: scrollTimer.restart()

    // The rows have no position until the popup that has just opened has laid them out.
    Timer {
        id: scrollTimer

        interval: 50
        onTriggered: full.showRequestedRow()
    }

    MoreOptionsMenu {
        id: moreMenu

        widget: full.widget
        settings: full.settings
        sharesMemory: full.resources.sharesMemory
        doNotDisturb: full.doNotDisturb
        notificationsOff: full.notificationsOff
        meterCount: full.meterCount
        notificationCount: full.notificationCount
    }

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

            PlasmaComponents.ToolButton {
                id: moreButton

                icon.name: "overflow-menu"
                text: "More options"
                display: PlasmaComponents.AbstractButton.IconOnly
                PlasmaComponents.ToolTip.text: text
                PlasmaComponents.ToolTip.visible: hovered
                PlasmaComponents.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: {
                    full.widget.refreshNotificationState();
                    moreMenu.open(moreButton);
                }
            }
        }
    }

    contentItem: Column {
        StopCard {
            id: stopCard

            width: parent.width
            question: full.stopQuestion
            widget: full.widget
        }

        ServiceBanner {
            id: serviceBanner

            width: parent.width
            banner: full.banner
            animate: full.animate
            widget: full.widget
        }

        PlasmaComponents.ScrollView {
            id: scroll

            width: parent.width
            height: full.listHeight

            PlasmaComponents.ScrollBar.horizontal.policy: PlasmaComponents.ScrollBar.AlwaysOff

            Column {
                id: list

                width: scroll.availableWidth
                opacity: full.hasSnapshot && full.snapshot.isLastKnown ? Allot.DIMMED : 1

                // A ListModel keeps each row's view while its key is shown, so a tick updates the
                // view in place and Kill? keeps its focus.
                Repeater {
                    id: rows

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

        ResourcesCard {
            id: resourcesCard

            width: parent.width
            view: full.resources
            animate: full.animate
            widget: full.widget
        }
    }
}
