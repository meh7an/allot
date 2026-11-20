import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// Test a command above the heavy-command rules, then one row per rule with its menu, Alt+Up,
// Alt+Down and Delete, then Add Rule… and Use Built-in Rules…. The rules rebuild only when they
// change, so focus survives the refresh, and a moved rule keeps the focus.
ColumnLayout {
    id: list

    property var host: null
    property var section: null
    property var matched: []
    property var items: []
    property string shownSignature: ""
    property int focusIndex: -1
    readonly property var rules: list.section.rules
    readonly property string status: {
        list.host.revision;
        return View.rulesStatus(list.rules);
    }

    function sync() {
        if (list.host.model === null) {
            return;
        }
        const signature = View.signature([list.rules.value, list.matched, list.host.readOnly]);
        if (signature === list.shownSignature) {
            return;
        }
        list.shownSignature = signature;
        list.items = View.ruleRowItems(list.rules, list.host.model.session, list.matched);
        const focus = list.focusIndex;
        list.focusIndex = -1;
        if (focus >= 0) {
            Qt.callLater(() => rows.itemAt(focus)?.forceActiveFocus(Qt.OtherFocusReason));
        }
    }

    function edit(index) {
        const draft = list.rules.edit(index);
        if (draft !== null) {
            list.host.openRuleDialog(list.rules, draft);
        }
    }

    function move(from, to) {
        const focus = View.moveRule(list.rules, from, to);
        if (focus !== null) {
            list.focusIndex = focus;
        }
        list.host.changed();
    }

    function remove(index) {
        const deleted = View.deleteRule(list.rules, index);
        list.focusIndex = deleted.focus;
        list.host.notice(deleted.text, deleted.undo);
        list.host.changed();
    }

    Layout.fillWidth: true
    spacing: Kirigami.Units.largeSpacing

    onMatchedChanged: list.sync()

    Connections {
        target: list.host

        function onRevisionChanged() {
            list.sync();
        }
    }

    Component.onCompleted: list.sync()

    // The page's own margins already hold this part; a section's would double them.
    SettingsSection {
        Layout.leftMargin: 0
        Layout.rightMargin: 0
        title: View.TEST_COMMAND_TITLE
        help: View.TEST_COMMAND_HELP
        enabled: !list.host.readOnly

        CommandTest {
            host: list.host
            rules: list.rules
            onTested: (test) => list.matched = View.matchedRules(test)
        }
    }

    SettingsSection {
        Layout.leftMargin: 0
        Layout.rightMargin: 0
        title: list.section.title
        help: list.status
        enabled: !list.host.readOnly

        Repeater {
            id: rows

            model: list.items

            QQC2.ItemDelegate {
                id: rule

                required property var modelData
                required property int index

                Layout.fillWidth: true
                Accessible.name: modelData.name
                onClicked: list.edit(index)
                Keys.onPressed: (event) => {
                    const alt = (event.modifiers & Qt.AltModifier) !== 0;
                    if (alt && event.key === Qt.Key_Up) {
                        list.move(index, index - 1);
                    } else if (alt && event.key === Qt.Key_Down) {
                        list.move(index, index + 1);
                    } else if (!alt && event.key === Qt.Key_Delete) {
                        list.remove(index);
                    } else {
                        return;
                    }
                    event.accepted = true;
                }

                contentItem: RowLayout {
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        QQC2.Label {
                            Layout.fillWidth: true
                            text: rule.modelData.title
                            elide: Text.ElideRight
                        }

                        QQC2.Label {
                            Layout.fillWidth: true
                            text: rule.modelData.subtitle
                            wrapMode: Text.Wrap
                            font: Kirigami.Theme.smallFont
                        }
                    }

                    QQC2.Label {
                        text: "Matches"
                        visible: rule.modelData.matches
                        color: Kirigami.Theme.highlightColor
                        font.bold: true
                    }

                    QQC2.ToolButton {
                        icon.name: "overflow-menu"
                        display: QQC2.AbstractButton.IconOnly
                        text: rule.modelData.menuName
                        Accessible.name: text
                        onClicked: menu.popup()

                        QQC2.Menu {
                            id: menu

                            QQC2.MenuItem {
                                text: "Edit…"
                                onTriggered: list.edit(rule.index)
                            }

                            QQC2.MenuItem {
                                text: "Move Up"
                                enabled: rule.modelData.canMoveUp
                                onTriggered: list.move(rule.index, rule.index - 1)
                            }

                            QQC2.MenuItem {
                                text: "Move Down"
                                enabled: rule.modelData.canMoveDown
                                onTriggered: list.move(rule.index, rule.index + 1)
                            }

                            QQC2.MenuItem {
                                text: "Delete"
                                onTriggered: list.remove(rule.index)
                            }
                        }
                    }
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: View.NO_RULES_TEXT
            visible: list.items.length === 0
            wrapMode: Text.Wrap
        }

        RowLayout {
            QQC2.Button {
                text: "Add Rule…"
                icon.name: "list-add"
                onClicked: list.host.openRuleDialog(list.rules, list.rules.add())
            }

            QQC2.Button {
                text: "Use Built-in Rules…"
                enabled: {
                    list.host.revision;
                    return list.rules.canReset;
                }
                onClicked: list.host.confirm(View.useBuiltInConfirmation(list.rules.value.length,
                    list.rules.control.default.length), () => list.rules.reset())
            }
        }
    }
}
