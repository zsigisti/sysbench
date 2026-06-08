import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.crucible.gui

ApplicationWindow {
    id: win
    width: 860
    height: 720
    visible: true
    title: "CRUCIBLE — crux"

    Controller { id: ctl }

    // Show the system report immediately on launch.
    Component.onCompleted: ctl.run("info", durationSpin.value, runsSpin.value)

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10

        // ── header ──────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                spacing: 0
                Label { text: "CRUCIBLE"; font.pixelSize: 30; font.bold: true }
                Label {
                    text: "Trial by fire — host-native CPU / memory / network / storage benchmark"
                    opacity: 0.7
                }
            }
            Item { Layout.fillWidth: true }
            // settings
            RowLayout {
                spacing: 6
                Label { text: "Duration" }
                SpinBox {
                    id: durationSpin
                    from: 1; to: 60; value: 10
                    enabled: !ctl.running
                    textFromValue: function(v) { return v + "s" }
                    implicitWidth: 96
                }
                Label { text: "Runs" }
                SpinBox {
                    id: runsSpin
                    from: 1; to: 9; value: 5
                    enabled: !ctl.running
                    implicitWidth: 80
                }
            }
        }

        // ── suite buttons ───────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Button { text: "Full Benchmark"; enabled: !ctl.running; onClicked: ctl.run("all",  durationSpin.value, runsSpin.value) }
            Button { text: "CPU";     enabled: !ctl.running; onClicked: ctl.run("cpu",  durationSpin.value, runsSpin.value) }
            Button { text: "Memory";  enabled: !ctl.running; onClicked: ctl.run("mem",  durationSpin.value, runsSpin.value) }
            Button { text: "Network"; enabled: !ctl.running; onClicked: ctl.run("net",  durationSpin.value, runsSpin.value) }
            Button { text: "Storage"; enabled: !ctl.running; onClicked: ctl.run("disk", durationSpin.value, runsSpin.value) }
            Item { Layout.fillWidth: true }
            Button { text: "System Info"; enabled: !ctl.running; onClicked: ctl.run("info", durationSpin.value, runsSpin.value) }
        }

        // ── actions ─────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Button {
                text: "Share"
                enabled: !ctl.running && ctl.output.length > 0
                onClicked: ctl.share()
            }
            Button {
                text: "Copy"
                enabled: ctl.output.length > 0
                onClicked: { results.selectAll(); results.copy(); results.deselect() }
            }
            Button {
                text: "Clear"
                enabled: !ctl.running && ctl.output.length > 0
                onClicked: ctl.clear()
            }
            TextField {
                Layout.fillWidth: true
                readOnly: true
                selectByMouse: true
                visible: ctl.share_url.length > 0
                text: ctl.share_url
                placeholderText: "paste.rs URL appears here after Share"
            }
        }

        // ── status ──────────────────────────────────────────────
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

        // ── results ─────────────────────────────────────────────
        Frame {
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollView {
                anchors.fill: parent
                clip: true
                TextArea {
                    id: results
                    readOnly: true
                    wrapMode: TextEdit.NoWrap
                    font.family: "monospace"
                    selectByMouse: true
                    text: ctl.output.length > 0
                        ? ctl.output
                        : "Pick a suite above to begin.\n\nResults match the `crux` CLI exactly. The Full Benchmark\nstreams each suite's results in as it finishes."
                }
            }
        }
    }
}
