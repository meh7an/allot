import QtQuick
import QtQuick.Controls as QQC2

// An icon-only button whose accessible name and tooltip say what it does and to what, such as
// "Edit cpu_heavy". The tooltip shows for keyboard focus too, not only the pointer.
QQC2.ToolButton {
    property string name: ""
    property string iconName: ""

    icon.name: iconName
    display: QQC2.AbstractButton.IconOnly
    text: name
    Accessible.name: name
    QQC2.ToolTip.text: name
    QQC2.ToolTip.visible: hovered || visualFocus
    QQC2.ToolTip.delay: 500
}
