import QtQuick
import Qt.labs.platform as Platform

import "../code/widget.mjs" as Allot

// The header's More options menu. Plasma's own QML menu has no submenus, and a Qt Quick one stays
// inside the popup on the Qt 6.6 that Plasma 6.0 runs, so this is a native QMenu. A tab before the
// count sets it at the item's right, where a menu shows shortcuts.
Platform.Menu {
    id: menu

    required property var widget
    required property var settings
    required property bool sharesMemory
    required property bool doNotDisturb
    required property bool notificationsOff
    required property string meterCount
    required property string notificationCount

    Platform.Menu {
        title: "Meters\t" + menu.meterCount

        Platform.MenuItem {
            text: "CPU"
            checkable: true
            checked: menu.settings.meters.cpu
            onTriggered: menu.widget.toggleMeter("cpu")
        }

        Platform.MenuItem {
            text: "RAM"
            checkable: true
            checked: menu.settings.meters.ram
            onTriggered: menu.widget.toggleMeter("ram")
        }

        Platform.MenuItem {
            text: "GPU"
            checkable: true
            checked: menu.settings.meters.gpu
            onTriggered: menu.widget.toggleMeter("gpu")
        }

        Platform.MenuItem {
            text: menu.sharesMemory ? "GPU Memory" : "VRAM"
            checkable: true
            checked: menu.settings.meters.vram
            onTriggered: menu.widget.toggleMeter("vram")
        }

        Platform.MenuItem {
            text: "Drives"
            checkable: true
            checked: menu.settings.meters.drives
            onTriggered: menu.widget.toggleMeter("drives")
        }

        Platform.MenuItem {
            text: "Drive Activity"
            checkable: true
            enabled: menu.settings.meters.drives
            checked: menu.settings.meters.drive_activity
            onTriggered: menu.widget.toggleMeter("drive_activity")
        }
    }

    Platform.Menu {
        title: "Notifications\t" + menu.notificationCount

        Platform.MenuItem {
            text: Allot.NOTIFICATIONS_OFF_TEXT
            enabled: false
            visible: menu.notificationsOff
        }

        Platform.MenuItem {
            text: "Open Notification Settings…"
            visible: menu.notificationsOff
            onTriggered: menu.widget.openNotificationSettings()
        }

        Platform.MenuItem {
            text: Allot.DO_NOT_DISTURB_TEXT
            enabled: false
            visible: menu.doNotDisturb
        }

        Platform.MenuSeparator {
            visible: menu.notificationsOff || menu.doNotDisturb
        }

        Platform.MenuItem {
            text: "Service Stopped"
            checkable: true
            checked: menu.settings.notifications.service_stopped
            onTriggered: menu.widget.toggleNotification("service_stopped")
        }

        Platform.MenuItem {
            text: "Running Long"
            checkable: true
            checked: menu.settings.notifications.running_long
            onTriggered: menu.widget.toggleNotification("running_long")
        }

        Platform.MenuItem {
            text: "Benchmark Finished"
            checkable: true
            checked: menu.settings.notifications.benchmark_finished
            onTriggered: menu.widget.toggleNotification("benchmark_finished")
        }

        Platform.MenuItem {
            text: "Job Finished"
            checkable: true
            checked: menu.settings.notifications.job_finished
            onTriggered: menu.widget.toggleNotification("job_finished")
        }

        Platform.MenuItem {
            text: "Killed Outside the Popup"
            checkable: true
            checked: menu.settings.notifications.killed_outside_flyout
            onTriggered: menu.widget.toggleNotification("killed_outside_flyout")
        }
    }

    Platform.MenuSeparator {}

    Platform.MenuItem {
        text: "Open Logs"
        onTriggered: menu.widget.openLogs()
    }
}
