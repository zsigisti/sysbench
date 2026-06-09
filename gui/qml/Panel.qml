// A titled surface panel. Children declared in the caller's scope are laid out
// in a column under the title. Used outside delegates, so referencing the
// caller's root id in child bindings is safe.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: panel
    property var pal
    property string title: ""
    default property alias content: body.data

    color: pal ? pal.surface : "#15181f"
    radius: pal ? pal.radius : 12
    border.color: pal ? pal.border : "#262b36"
    border.width: 1
    // Derive size from content so panels that aren't `Layout.fillHeight`
    // don't collapse to zero height (the +28 is the 14px margins on each side).
    implicitHeight: layout.implicitHeight + 28
    implicitWidth: layout.implicitWidth + 28

    ColumnLayout {
        id: layout
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        Label {
            visible: panel.title.length > 0
            text: panel.title
            color: pal ? pal.subtle : "#9aa1b1"
            font.bold: true
            font.pixelSize: 11
            font.capitalization: Font.AllUppercase
            font.letterSpacing: 1
        }
        ColumnLayout {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 8
        }
    }
}
