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
    property string iconSource: ""
    property string accessibleName: "Allot"
    property bool keepOpen: false

    readonly property string iconDirectory: Qt.resolvedUrl("../icons/").toString()

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
        root.snapshot = root.widget.model.snapshot;
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
    }

    compactRepresentation: MouseArea {
        hoverEnabled: true
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: root.accessibleName
        onClicked: root.expanded = !root.expanded
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space || event.key === Qt.Key_Return) {
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
        keepOpen: root.keepOpen
        iconDirectory: root.iconDirectory
        onKeepOpenToggled: {
            root.widget.toggleKeepOpen();
            root.refresh();
        }
    }

    Plasmoid.contextualActions: [
        PlasmaCore.Action {
            text: "Open Logs"
            icon.name: "document-open-folder"
            onTriggered: Qt.openUrlExternally("file://" + root.widget.paths.logs)
        }
    ]

    Component {
        id: timerComponent
        Timer {}
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
            connectSource: (command) => executable.connectSource(command),
            // XMLHttpRequest calls back only when send() runs in QML code, so it runs here.
            requests: {
                create: () => new XMLHttpRequest(),
                send: (xhr) => xhr.send(),
            },
            timers: root.timers,
            onChange: () => root.refresh(),
            log: (line) => console.log(line),
        });
        root.refresh();
        root.widget.start();
    }

    Component.onDestruction: root.widget.stop()
}
