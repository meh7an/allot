import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// A group of single settings, each in the row its kind gets, then the group's live sentence
// (DESIGN.md D10).
SettingsSection {
    id: group

    property var host: null
    property var section: null
    property string pageId: ""
    readonly property string live: {
        group.host.revision;
        if (group.section.live === null || group.host.model === null) {
            return "";
        }
        return group.host.model.live(group.pageId, group.section.live) ?? "";
    }

    title: group.section.title

    Repeater {
        // By index, so each row gets the session's own model, not a plain copy of it.
        model: group.section.settings.length

        ColumnLayout {
            id: slot

            required property int index
            readonly property var setting: group.section.settings[index]

            Layout.fillWidth: true

            Loader {
                Layout.fillWidth: true
                sourceComponent: {
                    switch (View.rowKind(slot.setting)) {
                    case "switch":
                        return switchRow;
                    case "duration_off":
                        return durationOffRow;
                    case "number":
                        return numberRow;
                    case "chips":
                        return chipsRow;
                    default:
                        return null;
                    }
                }
            }

            Component {
                id: switchRow

                SettingSwitch {
                    host: group.host
                    setting: slot.setting
                }
            }

            Component {
                id: durationOffRow

                SettingDurationOff {
                    host: group.host
                    setting: slot.setting
                }
            }

            Component {
                id: numberRow

                SettingNumber {
                    host: group.host
                    setting: slot.setting
                }
            }

            Component {
                id: chipsRow

                SettingChips {
                    host: group.host
                    setting: slot.setting
                    showLabel: slot.setting.label !== group.section.title
                }
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: group.live
        visible: text !== ""
        wrapMode: Text.Wrap
    }
}
