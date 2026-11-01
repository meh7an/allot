import QtCore
import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasma5support as P5Support
import org.kde.plasma.plasmoid

import "../code/widget.mjs" as Allot

// The panel icon and its popup. The logic lives in ../code, generated from apps/kde/src and the
// GNOME extension's modules; this file only hands it QML's timers, requests and file reads and
// mirrors its state into properties the views bind to.
PlasmoidItem {
    id: root

    property var widget: null
    property var snapshot: null
    property var resources: ({ isVisible: false, isExpanded: false, summary: "", meters: [],
        drives: [], showsSeparator: false, allOff: false, showsLegend: false,
        showsSharedLegend: false, sharesMemory: false })
    property var banner: null
    property var settings: ({ meters: {}, notifications: {} })
    property var serviceItem: ({ title: "", action: "none" })
    property string stopQuestion: ""
    property bool doNotDisturb: false
    property bool notificationsOff: false
    property string meterCount: ""
    property string notificationCount: ""
    property string iconSource: ""
    property string accessibleName: "Allot"
    property bool keepOpen: false
    property var rowData: ({})
    property bool animate: false
    property var scrollRequest: null

    readonly property string iconDirectory: Qt.resolvedUrl("../icons/").toString()
    readonly property string iconFolder: decodeURIComponent(iconDirectory.replace(/^file:\/\//, ""))

    // The poller's timers. It clears only timers that have not fired, so a timer is destroyed
    // exactly once.
    readonly property var timers: ({
        setTimeout: (callback, ms) => {
            const timer = timerComponent.createObject(root, { interval: ms });
            timer.triggered.connect(() => {
                timer.destroy();
                callback();
            });
            timer.start();
            return timer;
        },
        clearTimeout: (timer) => {
            timer.stop();
            timer.destroy();
        },
    })

    function refresh() {
        const model = root.widget.model;
        root.snapshot = model.snapshot;
        root.resources = model.resources;
        root.banner = model.banner;
        root.settings = model.settings;
        root.doNotDisturb = model.doNotDisturb;
        root.serviceItem = root.widget.serviceItem;
        root.stopQuestion = root.widget.stopQuestion;
        root.notificationsOff = root.widget.notificationsOff;
        root.meterCount = root.widget.meterCount;
        root.notificationCount = root.widget.notificationCount;
        root.iconSource = root.iconDirectory + root.widget.iconFile;
        root.accessibleName = root.widget.accessibleName;
        root.keepOpen = root.widget.keepOpen;
    }

    toolTipMainText: "Allot"
    toolTipSubText: root.snapshot === null ? "" : root.snapshot.header.summary
    // Plasma's Keep Open: the popup stays up while another window has focus.
    hideOnWindowDeactivate: !root.keepOpen

    onExpandedChanged: {
        if (root.widget === null) {
            return;
        }
        root.widget.setExpanded(root.expanded);
        root.refresh();
        if (root.expanded) {
            // The representation takes focus only once its window shows.
            Qt.callLater(() => root.fullRepresentationItem?.focusFirst());
        }
    }

    compactRepresentation: MouseArea {
        hoverEnabled: true
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: root.accessibleName
        onClicked: root.expanded = !root.expanded
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return
                    || event.key === Qt.Key_Enter) {
                root.expanded = !root.expanded;
                event.accepted = true;
            }
        }

        Kirigami.Icon {
            anchors.fill: parent
            source: root.iconSource
            active: parent.containsMouse
        }
    }

    fullRepresentation: FullRepresentation {
        snapshot: root.snapshot
        resources: root.resources
        banner: root.banner
        settings: root.settings
        stopQuestion: root.stopQuestion
        doNotDisturb: root.doNotDisturb
        notificationsOff: root.notificationsOff
        meterCount: root.meterCount
        notificationCount: root.notificationCount
        keepOpen: root.keepOpen
        iconDirectory: root.iconDirectory
        rowModel: listRows
        rowData: root.rowData
        animate: root.animate
        widget: root.widget
        scrollRequest: root.scrollRequest
        onKeepOpenToggled: {
            root.widget.toggleKeepOpen();
            root.refresh();
        }
    }

    // The summary, Stop Service or Start Service, and Open Logs. There is no Quit, because a widget
    // is removed from the panel, which Plasma adds to this menu.
    Plasmoid.contextualActions: [
        PlasmaCore.Action {
            text: root.snapshot === null ? "" : root.snapshot.header.summary
            enabled: false
        },
        PlasmaCore.Action {
            isSeparator: true
        },
        PlasmaCore.Action {
            text: root.serviceItem.title
            icon.name: root.serviceItem.action === "start" ? "media-playback-start"
                : "media-playback-stop"
            enabled: root.serviceItem.action !== "none"
            onTriggered: root.widget.pressServiceItem()
        },
        PlasmaCore.Action {
            text: "Open Logs"
            icon.name: "document-open-folder"
            onTriggered: root.widget.openLogs()
        }
    ]

    Component {
        id: timerComponent
        Timer {}
    }

    NotificationSender {
        id: notifications

        iconDirectory: root.iconFolder
    }

    ListModel {
        id: listRows
    }

    P5Support.DataSource {
        id: executable
        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            // The engine keeps a source and its last answer until it is disconnected, so the same
            // command can run again only after this.
            disconnectSource(source);
            root.widget.deliver(source, data);
        }
    }

    Component.onCompleted: {
        root.widget = new Allot.AllotWidget({
            homeUrl: StandardPaths.writableLocation(StandardPaths.HomeLocation).toString(),
            configUrl: StandardPaths.writableLocation(StandardPaths.ConfigLocation).toString(),
            dataUrl: StandardPaths.writableLocation(StandardPaths.GenericDataLocation).toString(),
            notifyRcUrl: Qt.resolvedUrl("../notifications/allot.notifyrc").toString(),
            connectSource: (command) => executable.connectSource(command),
            // XMLHttpRequest calls back only when send() runs in QML code, so it runs here.
            requests: {
                create: () => new XMLHttpRequest(),
                send: (xhr) => xhr.send(),
            },
            timers: root.timers,
            rows: listRows,
            showRows: (data, animate) => {
                root.animate = animate;
                root.rowData = data;
            },
            onChange: () => root.refresh(),
            sendNotification: (fields) => notifications.send(fields),
            expand: () => {
                root.expanded = true;
            },
            scrollTo: (key) => {
                root.scrollRequest = { key: key };
            },
            openUrl: (url) => Qt.openUrlExternally(url),
            announce: (text) => root.fullRepresentationItem?.announce(text),
            timeOfDay: (ms) => Qt.formatTime(new Date(ms),
                Qt.locale().timeFormat(Locale.ShortFormat)),
            log: (line) => console.log(line),
        });
        root.refresh();
        root.widget.start();
    }

    Component.onDestruction: root.widget.stop()
}
