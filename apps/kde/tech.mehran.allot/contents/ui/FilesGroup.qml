import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Files: the config file, opened in its editor or shown in its folder.
SettingsSection {
    id: files

    property var host: null
    readonly property string path: files.host.model === null ? ""
        : files.host.model.paths.config

    title: "Files"

    QQC2.Label {
        text: "Config file"
        Accessible.ignored: true
    }

    // Selectable, so it takes focus: named, or screen readers find an unnamed text (M7.1).
    Kirigami.SelectableLabel {
        Layout.fillWidth: true
        text: files.path
        font.family: "monospace"
        wrapMode: Text.WrapAnywhere
        Accessible.name: "Config file"
        Accessible.description: files.path
    }

    RowLayout {
        QQC2.Button {
            text: "Open File"
            icon.name: "document-open"
            onClicked: files.host.model.openFile()
        }

        QQC2.Button {
            text: "Show in Folder"
            icon.name: "folder-open"
            onClicked: files.host.showInFolder(files.path)
        }
    }
}
