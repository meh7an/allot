import QtQuick
import Qt.labs.platform as Platform

// The header's More options menu (DESIGN.md section 9): Configure Allot…, Stop Service or
// Start Service, and Open Logs. Meters and Notifications moved to the Settings window's Panel
// page. A Qt Quick menu stays inside the popup on the Qt 6.6 that Plasma 6.0 runs, so this is a
// native QMenu.
Platform.Menu {
    id: menu

    required property var widget
    required property var serviceItem

    signal settingsRequested()

    Platform.MenuItem {
        text: "Configure Allot…"
        icon.name: "configure"
        onTriggered: menu.settingsRequested()
    }

    Platform.MenuItem {
        text: menu.serviceItem.title
        icon.name: menu.serviceItem.action === "start" ? "media-playback-start"
            : "media-playback-stop"
        enabled: menu.serviceItem.action !== "none"
        onTriggered: menu.widget.pressServiceItem()
    }

    Platform.MenuSeparator {}

    Platform.MenuItem {
        text: "Open Logs"
        icon.name: "document-open-folder"
        onTriggered: menu.widget.openLogs()
    }
}
