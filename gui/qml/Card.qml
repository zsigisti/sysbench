// A titled surface panel. Children are laid out in a column inside it.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: card
    property var theme
    property string title: ""
    default property alias content: body.data

    color: theme ? theme.surface : "#191c23"
    radius: theme ? theme.radius : 10
    border.color: theme ? theme.border : "#2c313d"
    border.width: 1
    implicitHeight: col.implicitHeight + 24
    implicitWidth: col.implicitWidth + 24

    ColumnLayout {
        id: col
        anchors.fill: parent
        anchors.margins: 12
        spacing: 10

        Label {
            visible: card.title.length > 0
            text: card.title
            color: theme ? theme.subtle : "#9aa1b1"
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
