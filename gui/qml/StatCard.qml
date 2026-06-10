// A single key/value stat tile. Pure (only plain string/bool/object props) so
// it is safe inside Repeater/ListView delegates — no outer-id lookups.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: card
    property var pal
    property string label: ""
    property string value: "—"
    property bool accent: false

    radius: pal ? pal.radius : 12
    color: hover.hovered
        ? (pal ? pal.hoverBg : "#1d212b")
        : (pal ? pal.surface : "#15181f")
    border.color: pal ? pal.border : "#262b36"
    border.width: 1
    implicitHeight: 80

    Behavior on color { ColorAnimation { duration: 120 } }
    HoverHandler { id: hover }

    // ember tick down the left edge of accent cards
    Rectangle {
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        width: 3
        height: parent.height - 28
        radius: 2
        color: pal ? pal.accent : "#e0552b"
        visible: card.accent
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 14
        anchors.topMargin: 12
        anchors.bottomMargin: 12
        spacing: 4
        Label {
            text: card.label
            color: pal ? pal.subtle : "#9aa1b1"
            font.pixelSize: 11
            font.capitalization: Font.AllUppercase
            font.letterSpacing: 1.2
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        Label {
            text: card.value
            color: card.accent ? (pal ? pal.accent : "#e0552b") : (pal ? pal.text : "#e7e9ef")
            font.pixelSize: 20
            font.bold: true
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.fillHeight: true
            verticalAlignment: Text.AlignVCenter
        }
    }
}
