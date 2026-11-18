import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.plasmoid

import "../code/settings-client.mjs" as Client

// Host B: a window of the widget's own, built once and hidden on close, with instant apply: each
// burst of edits is one write against the revision on screen.
Kirigami.ApplicationWindow {
    id: window

    property var settings: null
    property var pending: ({})
    property bool sending: false

    function load() {
        io.client.load().then((outcome) => {
            io.log("load", { status: outcome.status, detail: outcome.detail });
            window.showOutcome(outcome, null);
        });
    }

    function send() {
        if (window.settings === null) {
            return;
        }
        if (window.sending) {
            // One write at a time; the next builds on this one's revision.
            applyTimer.restart();
            return;
        }
        const set = Client.changedValues(window.settings, window.pending);
        window.pending = {};
        if (Object.keys(set).length > 0) {
            window.write(set);
        }
    }

    function write(set) {
        window.sending = true;
        io.log("write-start", { set: set });
        io.client.apply(window.settings.revision, set, []).then((outcome) => {
            window.sending = false;
            io.log("write-done", { status: outcome.status });
            window.showOutcome(outcome, set);
        });
    }

    function showOutcome(outcome, mine) {
        if (outcome.settings !== null) {
            window.settings = outcome.settings;
            form.show(outcome.settings);
        }
        if (outcome.status === "conflict") {
            form.notify("Settings changed outside this window.", "Apply Mine",
                () => window.write(mine));
        } else if (outcome.status === "invalid") {
            form.notify(outcome.errors.map((error) => error.message).join("; "));
            form.show(window.settings);
        } else if (outcome.settings === null) {
            form.showDown(outcome.detail);
            if (mine !== null && window.settings !== null) {
                form.notify("Not saved: " + outcome.detail);
            }
        }
    }

    title: "Allot Settings Spike"
    width: Kirigami.Units.gridUnit * 34
    height: Kirigami.Units.gridUnit * 30
    minimumWidth: Kirigami.Units.gridUnit * 24
    minimumHeight: Kirigami.Units.gridUnit * 20

    onClosing: {
        io.log("window-closing");
        // A change made just before closing still goes out.
        if (applyTimer.running) {
            applyTimer.stop();
            window.send();
        }
    }
    onActiveChanged: io.log("window-active", { active: window.active })
    Component.onCompleted: {
        io.log("window-created");
        window.load();
    }
    Component.onDestruction: io.log("window-destroyed")

    pageStack.initialPage: Kirigami.ScrollablePage {
        title: "Settings"

        ColumnLayout {
            AdmissionForm {
                id: form

                Layout.fillWidth: true
                onEdited: (key, value) => {
                    window.pending[key] = value;
                    applyTimer.restart();
                }
                onRetry: window.load()
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                QQC2.CheckBox {
                    Kirigami.FormData.label: "Panel:"
                    text: "Show CPU (saved at once)"
                    checked: Plasmoid.configuration.showCpu
                    onToggled: Plasmoid.configuration.showCpu = checked
                }
            }
        }
    }

    SpikeIo {
        id: io

        where: "window"
    }

    Timer {
        id: applyTimer

        interval: 600
        onTriggered: window.send()
    }
}
