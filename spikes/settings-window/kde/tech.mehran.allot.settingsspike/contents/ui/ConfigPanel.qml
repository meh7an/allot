import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kcmutils as KCM
import org.kde.kirigami as Kirigami

// The widget's own settings: plain main.xml keys, which the dialog's Apply model handles by itself.
KCM.SimpleKCM {
    id: page

    property bool cfg_showCpu
    property bool cfg_sendThroughWidget
    property string cfg_pendingWrite

    Component.onCompleted: console.log("SettingsSpike " + JSON.stringify({
        event: "page-created",
        where: "dialog-panel",
    }))
    Component.onDestruction: console.log("SettingsSpike " + JSON.stringify({
        event: "page-destroyed",
        where: "dialog-panel",
    }))

    Kirigami.FormLayout {
        QQC2.CheckBox {
            Kirigami.FormData.label: "Meters:"
            text: "Show CPU"
            checked: page.cfg_showCpu
            onToggled: page.cfg_showCpu = checked
        }

        QQC2.CheckBox {
            Kirigami.FormData.label: "Spike:"
            text: "Send allotd's changes through the widget"
            checked: page.cfg_sendThroughWidget
            onToggled: page.cfg_sendThroughWidget = checked
        }
    }
}
