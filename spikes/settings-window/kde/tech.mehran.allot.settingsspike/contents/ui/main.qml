import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.plasma5support as P5Support
import org.kde.plasma.plasmoid

// The stand-in widget: opens Settings both ways, shows a widget setting as it changes, and sends
// the writes a config page hands over through the config.
PlasmoidItem {
    id: root

    // Built on first open and kept, so a reopen shows the same window.
    property var settingsWindow: null
    property string lastWrite: "none"
    // The hand-off from the config dialog; it changes once per Apply.
    readonly property string handedWrite: Plasmoid.configuration.pendingWrite

    function openDialog(from) {
        io.log("open-dialog", { from: from });
        Plasmoid.internalAction("configure").trigger();
    }

    function openWindow(from) {
        if (root.settingsWindow === null) {
            root.settingsWindow = windowComponent.createObject(root);
        }
        root.settingsWindow.show();
        root.settingsWindow.raise();
        root.settingsWindow.requestActivate();
        io.log("open-window", { from: from });
        focusProbe.restart();
    }

    function sendHandedWrite() {
        if (root.handedWrite === "") {
            return;
        }
        const write = JSON.parse(root.handedWrite);
        io.log("handed-write", { set: write.set });
        io.client.apply(write.revision, write.set, []).then((outcome) => {
            root.lastWrite = outcome.status + (outcome.detail ? ": " + outcome.detail : "");
            io.log("handed-write-done", { status: outcome.status });
        });
    }

    Plasmoid.icon: "preferences-system"
    Plasmoid.contextualActions: [
        PlasmaCore.Action {
            text: "Settings (Own Window)"
            icon.name: "configure"
            onTriggered: root.openWindow("context-menu")
        }
    ]
    onHandedWriteChanged: root.sendHandedWrite()
    Component.onCompleted: io.log("widget-created", { showCpu: Plasmoid.configuration.showCpu })

    fullRepresentation: ColumnLayout {
        Layout.minimumWidth: Kirigami.Units.gridUnit * 16

        PlasmaComponents.Button {
            Layout.fillWidth: true
            text: "Settings: Config Dialog"
            onClicked: root.openDialog("popup")
        }

        PlasmaComponents.Button {
            Layout.fillWidth: true
            text: "Settings: Own Window"
            onClicked: root.openWindow("popup")
        }

        PlasmaComponents.Label {
            text: "Show CPU: " + (Plasmoid.configuration.showCpu ? "on" : "off")
        }

        PlasmaComponents.Label {
            Layout.fillWidth: true
            text: "Last handed write: " + root.lastWrite
            wrapMode: Text.Wrap
        }
    }

    SpikeIo {
        id: io

        where: "widget"
    }

    Component {
        id: windowComponent

        SettingsWindow {}
    }

    Timer {
        id: focusProbe

        // Long enough for the window to map and for any activation to land.
        interval: 800
        onTriggered: io.log("window-state", {
            visible: root.settingsWindow.visible,
            active: root.settingsWindow.active,
            width: root.settingsWindow.width,
            height: root.settingsWindow.height,
        })
    }

    // Smoke-run hook: the kit writes a command to ~/settings-spike-command, read once a second.
    P5Support.DataSource {
        id: commands

        readonly property string read:
            "sh -c 'cat \"$HOME/settings-spike-command\" 2>/dev/null; rm -f \"$HOME/settings-spike-command\"'"

        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            disconnectSource(source);
            const command = data.stdout.trim();
            if (command === "open-dialog") {
                root.openDialog("command");
            } else if (command === "open-window") {
                root.openWindow("command");
            } else if (command === "close-window" && root.settingsWindow !== null) {
                root.settingsWindow.close();
            } else if (command === "expand") {
                root.expanded = true;
            }
        }
    }

    Timer {
        interval: 1000
        repeat: true
        running: true
        onTriggered: commands.connectSource(commands.read)
    }
}
