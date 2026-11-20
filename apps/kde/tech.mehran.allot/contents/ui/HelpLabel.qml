import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// A setting's help line, or a group's: small, wrapping rich text, whose key reads in monospace.
// Its text must come escaped, from the settings view module.
QQC2.Label {
    Layout.fillWidth: true
    wrapMode: Text.Wrap
    textFormat: Text.RichText
    font: Kirigami.Theme.smallFont
}
