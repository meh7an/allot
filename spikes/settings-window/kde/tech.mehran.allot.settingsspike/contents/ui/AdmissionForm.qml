import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-client.mjs" as Client

// allotd's settings as one form, shared by the config dialog page and the window. It only shows
// values and reports edits; each host decides when they are sent.
ColumnLayout {
    id: form

    // A value the user changed. It follows valueChanged, not valueModified, which a value set
    // through AT-SPI never emits; values put on screen from allotd, and the ones the controls
    // clamp to before the first load, are not edits.
    signal edited(string key, var value)
    signal retry()

    property string status: "Connecting..."
    property bool reachable: false
    property bool showing: false
    // The controls by key, which register themselves.
    property var controls: ({})
    property var onMessageAction: null

    function show(settings) {
        form.showing = true;
        for (const key in form.controls) {
            const control = form.controls[key];
            if (control.checkable) {
                control.checked = Boolean(settings.values[key]);
            } else {
                control.value = Number(settings.values[key]);
            }
        }
        form.showing = false;
        form.reachable = true;
        form.status = "Connected, revision " + settings.revision
            + (settings.restart_pending ? ". Restart allotd to use the new port." : "");
    }

    function report(field, value) {
        if (field && form.reachable && !form.showing) {
            form.edited(field.key, value);
        }
    }

    function showDown(detail) {
        form.reachable = false;
        form.status = "allotd is not answering: " + detail;
    }

    function notify(text, actionText, onAction) {
        message.text = text;
        messageAction.text = actionText || "";
        messageAction.visible = actionText !== undefined;
        form.onMessageAction = onAction || null;
        message.visible = true;
    }

    Kirigami.InlineMessage {
        id: message

        Layout.fillWidth: true
        visible: false
        showCloseButton: true
        actions: [
            Kirigami.Action {
                id: messageAction

                visible: false
                onTriggered: {
                    message.visible = false;
                    if (form.onMessageAction !== null) {
                        form.onMessageAction();
                    }
                }
            }
        ]
    }

    Kirigami.FormLayout {
        Layout.fillWidth: true

        RowLayout {
            Kirigami.FormData.label: "allotd:"

            QQC2.Label {
                Layout.maximumWidth: Kirigami.Units.gridUnit * 18
                text: form.status
                wrapMode: Text.Wrap
            }

            QQC2.Button {
                text: "Retry"
                onClicked: form.retry()
            }
        }

        Repeater {
            model: Client.SPIKE_FIELDS

            delegate: Loader {
                required property var modelData

                Kirigami.FormData.label: modelData.title + ":"
                enabled: form.reachable
                sourceComponent: modelData.kind === "switch" ? switchControl : numberControl
                onLoaded: {
                    item.field = modelData;
                    form.controls[modelData.key] = item;
                }
            }
        }
    }

    Component {
        id: numberControl

        QQC2.SpinBox {
            property var field

            from: field ? field.min : 0
            to: field ? field.max : 100
            stepSize: field ? field.step : 1
            editable: true
            textFromValue: (value) => String(value)
            valueFromText: (text) => parseInt(text, 10)
            Accessible.name: field ? field.title : ""
            QQC2.ToolTip.text: field ? field.subtitle : ""
            QQC2.ToolTip.visible: hovered
            onValueChanged: form.report(field, value)
        }
    }

    Component {
        id: switchControl

        QQC2.CheckBox {
            property var field

            text: field ? field.subtitle : ""
            Accessible.name: field ? field.title : ""
            onCheckedChanged: form.report(field, checked)
        }
    }
}
