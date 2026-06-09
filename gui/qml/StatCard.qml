// A single key/value stat tile. Pure (only plain string/bool/object props) so
// it is safe inside Repeater/ListView delegates — no outer-id lookups.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    property var pal
    property string label: ""
    property string value: "—"
    property bool accent: false

    radius: pal ? pal.radius : 12
    color: pal ? pal.surface : "#15181f"
    border.color: pal ? pal.border : "#262b36"
    border.width: 1
    implicitHeight: 78

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 14
        anchors.rightMargin: 14
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        spacing: 4
        Label {
            text: label
            color: pal ? pal.subtle : "#9aa1b1"
            font.pixelSize: 11
            font.capitalization: Font.AllUppercase
            font.letterSpacing: 1
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        Label {
            text: value
            color: accent ? (pal ? pal.accent : "#e0552b") : (pal ? pal.text : "#e7e9ef")
            font.pixelSize: 19
            font.bold: true
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.fillHeight: true
            verticalAlignment: Text.AlignVCenter
        }
    }
}
