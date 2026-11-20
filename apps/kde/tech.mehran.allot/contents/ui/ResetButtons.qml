import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

import "../code/settings-view.mjs" as View

// Reset Page… at the bottom of each allotd page, and on Service Reset All Service Settings…
// (DESIGN.md section 6). Both ask first, and are off with nothing to reset.
RowLayout {
    id: reset

    property var host: null
    property var section: null
    property string pageId: ""
    property string pageLabel: ""
    readonly property bool canResetPage: {
        reset.host.revision;
        return reset.host.model !== null && reset.host.model.changedOn(reset.pageId).length > 0;
    }
    readonly property bool canResetAll: {
        reset.host.revision;
        return reset.host.model !== null && reset.host.model.canResetAll;
    }

    Layout.fillWidth: true
    Layout.margins: Kirigami.Units.largeSpacing

    QQC2.Button {
        text: "Reset Page…"
        enabled: reset.canResetPage
        Accessible.description: reset.canResetPage ? "" : View.NOTHING_TO_RESET
        onClicked: reset.host.model.resetPage(reset.pageId, reset.pageLabel)
    }

    QQC2.Button {
        text: "Reset All Service Settings…"
        visible: reset.section.all
        enabled: reset.canResetAll
        Accessible.description: reset.canResetAll ? "" : View.NOTHING_TO_RESET
        onClicked: reset.host.model.resetAll()
    }

    Item {
        Layout.fillWidth: true
    }
}
