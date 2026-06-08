import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.crucible.gui

ApplicationWindow {
    id: win
    width: 780
    height: 640
    visible: true
    title: "CRUCIBLE — crux"

    Controller { id: ctl }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        Label {
            text: "CRUCIBLE"
            font.pixelSize: 30
            font.bold: true
        }
        Label {
            text: "Trial by fire — host-native CPU / memory / network / storage benchmark"
            opacity: 0.7
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Button { text: "Full Benchmark"; enabled: !ctl.running; onClicked: ctl.run("all") }
            Button { text: "CPU";     enabled: !ctl.running; onClicked: ctl.run("cpu") }
            Button { text: "Memory";  enabled: !ctl.running; onClicked: ctl.run("mem") }
            Button { text: "Network"; enabled: !ctl.running; onClicked: ctl.run("net") }
            Button { text: "Storage"; enabled: !ctl.running; onClicked: ctl.run("disk") }
            Item { Layout.fillWidth: true }
            Button { text: "System Info"; enabled: !ctl.running; onClicked: ctl.run("info") }
        }

        RowLayout {
            spacing: 8
            BusyIndicator {
                running: ctl.running
                visible: ctl.running
                implicitWidth: 22
                implicitHeight: 22
            }
            Label { text: ctl.status }
        }

        Frame {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollView {
                anchors.fill: parent
                clip: true
                TextArea {
                    readOnly: true
                    wrapMode: TextEdit.NoWrap
                    font.family: "monospace"
                    selectByMouse: true
                    text: ctl.output.length > 0
                        ? ctl.output
                        : "Pick a suite above to begin.\n\nBenchmark results match the `crux` CLI exactly.\nFull runs take a few minutes (10s × 5 runs per CPU test)."
                }
            }
        }
    }
}
