import QtQuick
import QtQuick.Controls as QQC2

// A field's reset button: an icon whose accessible name says what goes back to what, such as
// "Reset Headroom to 15 %".
QQC2.ToolButton {
    property string name: ""

    icon.name: "edit-undo"
    display: QQC2.AbstractButton.IconOnly
    text: name
    Accessible.name: name
    QQC2.ToolTip.text: name
    QQC2.ToolTip.visible: hovered
    QQC2.ToolTip.delay: 500
}
