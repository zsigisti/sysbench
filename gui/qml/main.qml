import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs
import com.crucible.gui

ApplicationWindow {
    id: win
    width: 1060
    height: 760
    minimumWidth: 840
    minimumHeight: 560
    visible: true
    title: "CRUCIBLE — crux"
    color: win.bg

    Controller { id: ctl }

    // ── palette (on the ROOT id so it resolves inside delegates too) ────────
    readonly property bool dark: ctl.dark
    readonly property color bg:       dark ? "#0e1014" : "#f4f5f7"
    readonly property color surface:  dark ? "#15181f" : "#ffffff"
    readonly property color surface2: dark ? "#1d212b" : "#eceef2"
    readonly property color border:   dark ? "#262b36" : "#d9dce3"
    readonly property color text:     dark ? "#e7e9ef" : "#1a1c21"
    readonly property color subtle:   dark ? "#9aa1b1" : "#5d636f"
    readonly property color accent:   "#e0552b"
    readonly property color accent2:  "#f0a020"
    readonly property color good:     dark ? "#46c95a" : "#1f9d38"
    readonly property color bad:      dark ? "#f0705a" : "#d23b27"
    readonly property color hoverBg:  dark ? "#1d212b" : "#e9ebf0"
    readonly property color activeBg: dark ? "#232836" : "#e4e7ee"
    readonly property int radius: 12
    readonly property string mono: "monospace"

    // Theme every standard (Basic-style) control via the inherited palette, so
    // SpinBox/ComboBox/TextField/Dialog text isn't black-on-dark. Children
    // inherit this palette automatically.
    palette.window: win.bg
    palette.windowText: win.text
    palette.base: win.surface
    palette.alternateBase: win.surface2
    palette.text: win.text
    palette.button: win.surface2
    palette.buttonText: win.text
    palette.mid: win.border
    palette.dark: win.border
    palette.light: win.surface2
    palette.highlight: win.accent
    palette.highlightedText: "#ffffff"
    palette.placeholderText: win.subtle
    palette.toolTipBase: win.surface
    palette.toolTipText: win.text

    property int tab: 0
    readonly property var tabTitles: ["Benchmark", "System", "History", "Settings"]

    // parsed views of the controller's JSON string properties
    property var facts: ctl.sys_facts.length ? JSON.parse(ctl.sys_facts) : ({})
    property var histList: ctl.history.length ? JSON.parse(ctl.history) : []
    property var ana: ctl.analysis.length ? JSON.parse(ctl.analysis) : ({})

    function fnum(v, d) { return (v === null || v === undefined) ? "—" : Number(v).toFixed(d === undefined ? 0 : d) }

    // ── reusable styled controls ────────────────────────────────────────────
    component Pill: Button {
        id: pb
        property color base: win.surface2
        property color fg: win.text
        font.pixelSize: 13
        padding: 9; leftPadding: 16; rightPadding: 16
        background: Rectangle {
            radius: 8
            color: !pb.enabled ? win.surface2
                 : pb.down ? Qt.darker(pb.base, 1.25)
                 : pb.hovered ? Qt.lighter(pb.base, win.dark ? 1.3 : 1.04) : pb.base
            border.color: win.border
            border.width: 1
            opacity: pb.enabled ? 1 : 0.45
        }
        contentItem: Label {
            text: pb.text; color: pb.fg
            opacity: pb.enabled ? 1 : 0.6
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
    component Primary: Pill { base: win.accent; fg: "#ffffff" }

    component NavItem: Button {
        id: nv
        property int index: 0
        checkable: true
        checked: win.tab === index
        onClicked: win.tab = index
        Layout.fillWidth: true
        padding: 10
        background: Rectangle {
            radius: 8
            color: nv.checked ? win.activeBg : (nv.hovered ? win.hoverBg : "transparent")
        }
        contentItem: RowLayout {
            spacing: 10
            Rectangle { width: 3; Layout.preferredHeight: 16; radius: 2; color: nv.checked ? win.accent : "transparent" }
            Label { text: nv.text; color: nv.checked ? win.text : win.subtle; font.bold: nv.checked; font.pixelSize: 14; Layout.fillWidth: true }
        }
    }

    // ── dialogs ─────────────────────────────────────────────────────────────
    FileDialog {
        id: saveReport
        title: "Export report (.txt)"; fileMode: FileDialog.SaveFile
        defaultSuffix: "txt"; nameFilters: ["Text (*.txt)", "All files (*)"]
        onAccepted: ctl.export_report(selectedFile)
    }
    FileDialog {
        id: saveJson
        title: "Export results (.json)"; fileMode: FileDialog.SaveFile
        defaultSuffix: "json"; nameFilters: ["JSON (*.json)", "All files (*)"]
        onAccepted: ctl.export_json(selectedFile)
    }
    Dialog {
        id: confirmUninstall
        anchors.centerIn: parent; modal: true
        title: "Uninstall CRUCIBLE?"
        standardButtons: Dialog.Cancel | Dialog.Ok
        property bool purge: false
        onAccepted: ctl.uninstall(purge)
        contentItem: ColumnLayout {
            spacing: 12
            Label { text: "Removes the installed crux / crux-gui binaries, the man\npage, shell completions, and the desktop entry."; color: win.text }
            CheckBox { id: purgeBox; text: "Also delete local run history"; onCheckedChanged: confirmUninstall.purge = checked }
        }
    }

    // ── shell: left rail + content ──────────────────────────────────────────
    RowLayout {
        anchors.fill: parent
        spacing: 0

        // left rail
        Rectangle {
            Layout.fillHeight: true
            Layout.preferredWidth: 216
            color: win.surface
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: win.border }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 6

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10
                    Image { source: "logo.svg"; sourceSize.width: 34; sourceSize.height: 34; fillMode: Image.PreserveAspectFit }
                    ColumnLayout {
                        spacing: 0
                        Label { text: "CRUCIBLE"; color: win.text; font.pixelSize: 17; font.bold: true }
                        Label { text: "trial by fire"; color: win.subtle; font.pixelSize: 10; font.capitalization: Font.AllUppercase; font.letterSpacing: 1 }
                    }
                }
                Item { Layout.preferredHeight: 12 }

                NavItem { text: "Benchmark"; index: 0 }
                NavItem { text: "System";    index: 1 }
                NavItem { text: "History";   index: 2 }
                NavItem { text: "Settings";  index: 3 }

                Item { Layout.fillHeight: true }

                RowLayout {
                    Layout.fillWidth: true
                    Label { text: "Theme"; color: win.subtle; Layout.fillWidth: true }
                    Label { text: win.dark ? "☾" : "☀"; color: win.subtle; font.pixelSize: 15 }
                    Switch { checked: win.dark; onToggled: ctl.dark = checked }
                }
            }
        }

        // content
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // page header
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 66
                color: "transparent"
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 22; anchors.rightMargin: 22
                    Label { text: win.tabTitles[win.tab]; color: win.text; font.pixelSize: 22; font.bold: true }
                    Item { Layout.fillWidth: true }
                    RowLayout {
                        visible: win.tab === 0
                        spacing: 8
                        Label { text: "Duration"; color: win.subtle }
                        SpinBox { id: durationSpin; from: 1; to: 60; value: 10; enabled: !ctl.running
                            textFromValue: function(v) { return v + "s" }; implicitWidth: 96 }
                        Label { text: "Runs"; color: win.subtle }
                        SpinBox { id: runsSpin; from: 1; to: 9; value: 5; enabled: !ctl.running; implicitWidth: 74 }
                    }
                }
                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: win.border }
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: win.tab

                // ───────────────────────── Benchmark ──────────────────────────
                ColumnLayout {
                    spacing: 14
                    property int pad: 22
                    Item { Layout.preferredHeight: 8 }

                    RowLayout {
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        spacing: 8
                        Primary { text: "Full Benchmark"; enabled: !ctl.running; onClicked: ctl.run("all",  durationSpin.value, runsSpin.value) }
                        Pill { text: "CPU";     enabled: !ctl.running; onClicked: ctl.run("cpu",  durationSpin.value, runsSpin.value) }
                        Pill { text: "Memory";  enabled: !ctl.running; onClicked: ctl.run("mem",  durationSpin.value, runsSpin.value) }
                        Pill { text: "Network"; enabled: !ctl.running; onClicked: ctl.run("net",  durationSpin.value, runsSpin.value) }
                        Pill { text: "Storage"; enabled: !ctl.running; onClicked: ctl.run("disk", durationSpin.value, runsSpin.value) }
                        Item { Layout.fillWidth: true }
                        Pill { text: "System Info"; enabled: !ctl.running; onClicked: ctl.run("info", durationSpin.value, runsSpin.value) }
                    }

                    RowLayout {
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        spacing: 8
                        Primary { text: "Submit ↑"; enabled: !ctl.running && ctl.has_results; onClicked: ctl.submit() }
                        Pill { text: "Share"; enabled: !ctl.running && (ctl.has_results || ctl.output.length > 0); onClicked: ctl.share() }
                        Pill { text: "Copy"; enabled: ctl.output.length > 0; onClicked: { results.selectAll(); results.copy(); results.deselect() } }
                        Pill { text: "Export .txt"; enabled: ctl.output.length > 0; onClicked: saveReport.open() }
                        Pill { text: "Export .json"; enabled: ctl.has_results; onClicked: saveJson.open() }
                        Item { Layout.fillWidth: true }
                        Pill { text: "Clear"; enabled: !ctl.running && ctl.output.length > 0; onClicked: ctl.clear() }
                    }

                    // share url
                    Panel {
                        pal: win; Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        visible: ctl.share_url.length > 0
                        RowLayout {
                            Layout.fillWidth: true
                            Label { text: ctl.backend === "crux" ? "Result" : "Paste"; color: win.subtle }
                            TextField {
                                Layout.fillWidth: true; readOnly: true; selectByMouse: true
                                text: ctl.share_url; color: win.text
                                background: Rectangle { color: win.surface2; radius: 6; border.color: win.border }
                            }
                        }
                    }

                    // status
                    RowLayout {
                        Layout.leftMargin: 22; Layout.rightMargin: 22
                        spacing: 8
                        BusyIndicator { running: ctl.running; visible: ctl.running; implicitWidth: 20; implicitHeight: 20 }
                        Label { text: ctl.status; color: win.subtle }
                    }

                    // results
                    Panel {
                        pal: win; title: "Results"
                        Layout.fillWidth: true; Layout.fillHeight: true
                        Layout.leftMargin: 22; Layout.rightMargin: 22; Layout.bottomMargin: 22
                        Item {
                            Layout.fillWidth: true; Layout.fillHeight: true
                            // faint logo watermark on the empty state
                            Image {
                                anchors.centerIn: parent
                                source: "logo.svg"; sourceSize.width: 120; sourceSize.height: 120
                                opacity: 0.05; visible: ctl.output.length === 0
                            }
                            ScrollView {
                                anchors.fill: parent; clip: true
                                TextArea {
                                    id: results
                                    readOnly: true; wrapMode: TextEdit.NoWrap
                                    font.family: win.mono; font.pixelSize: 13
                                    color: win.text; selectByMouse: true; background: null
                                    text: ctl.output.length > 0 ? ctl.output
                                        : "Pick a suite above to begin.\n\nThe Full Benchmark streams each suite's results in as it\nfinishes, records the run to local history, and lets you\nShare, Submit, or Export the JSON."
                                }
                            }
                        }
                    }
                }

                // ───────────────────────── System ─────────────────────────────
                ColumnLayout {
                    spacing: 14
                    Item { Layout.preferredHeight: 8 }
                    GridLayout {
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        columns: 3; columnSpacing: 12; rowSpacing: 12
                        Repeater {
                            model: [
                                { k: "Processor", v: win.facts.cpu_model || "—" },
                                { k: "Logical cores", v: (win.facts.logical_cores !== undefined ? String(win.facts.logical_cores) : "—") },
                                { k: "Memory", v: (win.facts.ram_mib ? (win.facts.ram_mib/1024).toFixed(1) + " GiB" : "—") },
                                { k: "Kernel", v: win.facts.kernel || "—" },
                                { k: "OS", v: win.facts.os || "—" }
                            ]
                            delegate: StatCard { pal: win; label: modelData.k; value: modelData.v; Layout.fillWidth: true; Layout.minimumWidth: 200 }
                        }
                    }
                    Panel {
                        pal: win; title: "Deep report"
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                Layout.fillWidth: true; wrapMode: Text.WordWrap; color: win.subtle
                                text: "The full `crux info` report — cache hierarchy, every disk, thermals, batteries, per-interface IPs — opens in the Benchmark pane."
                            }
                            Primary { text: "Open full report"; enabled: !ctl.running; onClicked: { win.tab = 0; ctl.run("info", durationSpin.value, runsSpin.value) } }
                        }
                    }
                    Item { Layout.fillHeight: true }
                }

                // ───────────────────────── History ────────────────────────────
                ColumnLayout {
                    spacing: 14
                    Item { Layout.preferredHeight: 8 }

                    GridLayout {
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        columns: 4; columnSpacing: 12; rowSpacing: 12
                        Repeater {
                            model: [
                                { k: "Runs recorded", v: win.fnum(win.ana.count), a: false },
                                { k: "Best CPU (MT)", v: win.fnum(win.ana.best_mt), a: true },
                                { k: "Best CPU (ST)", v: win.fnum(win.ana.best_st), a: true },
                                { k: "Best Triad", v: win.fnum(win.ana.best_triad, 1) + " GB/s", a: true }
                            ]
                            delegate: StatCard { pal: win; label: modelData.k; value: modelData.v; accent: modelData.a; Layout.fillWidth: true }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        spacing: 8
                        Pill { text: "Refresh"; onClicked: ctl.refresh_history() }
                        Item { Layout.fillWidth: true }
                        Label { text: "Compare"; color: win.subtle }
                        ComboBox { id: cmpA; Layout.preferredWidth: 220; model: win.histList; textRole: "headline" }
                        ComboBox { id: cmpB; Layout.preferredWidth: 220; model: win.histList; textRole: "headline" }
                        Primary {
                            text: "Compare"; enabled: win.histList.length >= 2
                            onClicked: {
                                var a = win.histList[cmpA.currentIndex];
                                var b = win.histList[cmpB.currentIndex];
                                if (a && b) ctl.compare_runs(a.id, b.id);
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true; Layout.fillHeight: true
                        Layout.leftMargin: 22; Layout.rightMargin: 22; Layout.bottomMargin: 22
                        spacing: 12

                        Panel {
                            pal: win; title: "Recorded runs"
                            Layout.preferredWidth: 1; Layout.fillWidth: true; Layout.fillHeight: true
                            ListView {
                                Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                                model: win.histList; spacing: 6
                                delegate: Rectangle {
                                    width: ListView.view ? ListView.view.width : 0
                                    height: 52; radius: 8
                                    color: win.surface2; border.color: win.border
                                    ColumnLayout {
                                        anchors.fill: parent; anchors.margins: 9; spacing: 2
                                        Label { text: modelData.cpu || "—"; color: win.text; font.bold: true; elide: Text.ElideRight; Layout.fillWidth: true }
                                        Label { color: win.subtle; font.pixelSize: 11
                                            text: modelData.when + "   ·   MT " + win.fnum(modelData.mt) + " / ST " + win.fnum(modelData.st) }
                                    }
                                }
                            }
                            Label {
                                visible: win.histList.length === 0
                                text: "No runs yet — run a benchmark to record one."
                                color: win.subtle
                            }
                        }

                        Panel {
                            pal: win; title: "Comparison"
                            Layout.preferredWidth: 1; Layout.fillWidth: true; Layout.fillHeight: true
                            ScrollView {
                                Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                                TextArea {
                                    readOnly: true; wrapMode: TextEdit.NoWrap
                                    font.family: win.mono; font.pixelSize: 12
                                    color: win.text; background: null
                                    text: ctl.compare_text.length > 0 ? ctl.compare_text
                                        : "Choose two runs above and press Compare.\nDeltas are computed per metric."
                                }
                            }
                        }
                    }
                }

                // ───────────────────────── Settings ───────────────────────────
                ColumnLayout {
                    spacing: 14
                    Item { Layout.preferredHeight: 8 }
                    Panel {
                        pal: win; title: "Appearance"
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        RowLayout {
                            Layout.fillWidth: true
                            Label { text: "Dark theme"; color: win.text; Layout.fillWidth: true }
                            Switch { checked: win.dark; onToggled: ctl.dark = checked }
                        }
                    }
                    Panel {
                        pal: win; title: "Score server"
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        Label {
                            Layout.fillWidth: true; wrapMode: Text.WordWrap; color: win.subtle
                            text: "Submit sends results to the CRUCIBLE score server (default\nhttps://crux.mmzsigmond.me). Override with the CRUX_SERVER\nenvironment variable. Share falls back to paste.rs if the\nserver is unreachable."
                        }
                    }
                    Panel {
                        pal: win; title: "Danger zone"
                        Layout.fillWidth: true; Layout.leftMargin: 22; Layout.rightMargin: 22
                        RowLayout {
                            Layout.fillWidth: true
                            ColumnLayout {
                                Layout.fillWidth: true; spacing: 2
                                Label { text: "Uninstall CRUCIBLE"; color: win.text; font.bold: true }
                                Label { text: "Removes binaries, man page, completions and the desktop entry."; color: win.subtle; font.pixelSize: 11 }
                            }
                            Pill { text: "Uninstall…"; base: win.bad; fg: "#ffffff"; enabled: !ctl.running; onClicked: confirmUninstall.open() }
                        }
                    }
                    Item { Layout.fillHeight: true }
                }
            }
        }
    }

    Component.onCompleted: ctl.refresh_history()
}
