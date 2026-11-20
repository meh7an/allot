import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// What fits on this machine: a line per class from allotd's fit preview, then its caption.
SettingsSection {
    id: fits

    property var host: null
    readonly property var view: {
        fits.host.revision;
        return fits.host.model === null ? { rows: [], caption: "" } : fits.host.model.whatFits;
    }

    title: View.WHAT_FITS_TITLE

    Repeater {
        model: fits.view.rows

        ColumnLayout {
            required property var modelData

            Layout.fillWidth: true
            spacing: 0

            QQC2.Label {
                Layout.fillWidth: true
                text: modelData.label
                wrapMode: Text.Wrap
            }

            QQC2.Label {
                Layout.fillWidth: true
                text: modelData.text
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: fits.view.caption
        visible: text !== ""
        wrapMode: Text.Wrap
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
