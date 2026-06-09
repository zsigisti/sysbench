import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import com.crucible.gui

ApplicationWindow {
    id: win
    width: 1000
    height: 780
    minimumWidth: 720
    minimumHeight: 560
    visible: true
    title: "CRUCIBLE — crux"

    Controller { id: ctl }
    Theme { id: theme; dark: ctl.dark }

    color: theme.bg
    property int tab: 0

    // Parsed views of the controller's JSON string properties.
    property var facts: ctl.sys_facts.length ? JSON.parse(ctl.sys_facts) : ({})
    property var histList: ctl.history.length ? JSON.parse(ctl.history) : []
    property var ana: ctl.analysis.length ? JSON.parse(ctl.analysis) : ({})

    function fnum(v, d) { return (v === null || v === undefined) ? "—" : Number(v).toFixed(d === undefined ? 0 : d) }

    // ── reusable styled controls ───────────────────────────────────────────
    component Pill: Button {
        id: pb
        property color base: theme.surface2
        property color fg: theme.text
        enabled: true
        font.pixelSize: 13
        padding: 9
        leftPadding: 16; rightPadding: 16
        background: Rectangle {
            radius: 8
            color: !pb.enabled ? Qt.darker(pb.base, 1.1)
                 : pb.down ? Qt.darker(pb.base, 1.25)
                 : pb.hovered ? Qt.lighter(pb.base, theme.dark ? 1.25 : 1.04) : pb.base
            border.color: theme.border
            border.width: 1
            opacity: pb.enabled ? 1 : 0.5
        }
        contentItem: Label {
            text: pb.text; color: pb.fg
            opacity: pb.enabled ? 1 : 0.6
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
    component Primary: Pill { base: theme.accent; fg: "#ffffff" }

    component TabButton2: Button {
        id: tb
        property int index: 0
        checkable: true
        checked: win.tab === index
        onClicked: win.tab = index
        font.pixelSize: 14
        padding: 10; leftPadding: 18; rightPadding: 18
        background: Rectangle {
            color: tb.checked ? theme.surface : "transparent"
            radius: 8
            border.color: tb.checked ? theme.border : "transparent"
            border.width: 1
        }
        contentItem: Label {
            text: tb.text
            color: tb.checked ? theme.accent : theme.subtle
            font.bold: tb.checked
            horizontalAlignment: Text.AlignHCenter
        }
    }

    // ── file dialogs (export) ──────────────────────────────────────────────
    FileDialog {
        id: saveReport
        title: "Export report (.txt)"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "txt"
        nameFilters: ["Text (*.txt)", "All files (*)"]
        onAccepted: ctl.export_report(selectedFile)
    }
    FileDialog {
        id: saveJson
        title: "Export results (.json)"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: ["JSON (*.json)", "All files (*)"]
        onAccepted: ctl.export_json(selectedFile)
    }

    // ── uninstall confirm ──────────────────────────────────────────────────
    Dialog {
        id: confirmUninstall
        anchors.centerIn: parent
        modal: true
        title: "Uninstall CRUCIBLE?"
        standardButtons: Dialog.Cancel | Dialog.Ok
        property bool purge: false
        onAccepted: ctl.uninstall(purge)
        contentItem: ColumnLayout {
            spacing: 10
            Label {
                text: "This removes the installed crux / crux-gui binaries, the\nman page, shell completions, and the desktop entry."
                color: theme.text
            }
            CheckBox {
                id: purgeBox
                text: "Also delete local run history"
                onCheckedChanged: confirmUninstall.purge = checked
            }
        }
    }

    // ── layout ─────────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 14

        // header
        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            Rectangle {
                width: 42; height: 42; radius: 10
                gradient: Gradient {
                    GradientStop { position: 0.0; color: theme.accent2 }
                    GradientStop { position: 1.0; color: theme.accent }
                }
                Label { anchors.centerIn: parent; text: "C"; color: "white"; font.bold: true; font.pixelSize: 24 }
            }
            ColumnLayout {
                spacing: 0
                Label { text: "CRUCIBLE"; color: theme.text; font.pixelSize: 24; font.bold: true }
                Label { text: "Trial by fire — CPU · memory · network · storage"; color: theme.subtle; font.pixelSize: 12 }
            }
            Item { Layout.fillWidth: true }
            RowLayout {
                spacing: 8
                Label { text: "Duration"; color: theme.subtle }
                SpinBox {
                    id: durationSpin
                    from: 1; to: 60; value: 10
                    enabled: !ctl.running
                    textFromValue: function(v) { return v + "s" }
                    implicitWidth: 96
                }
                Label { text: "Runs"; color: theme.subtle }
                SpinBox { id: runsSpin; from: 1; to: 9; value: 5; enabled: !ctl.running; implicitWidth: 76 }
                ToolSeparator {}
                Label { text: "☾"; color: theme.subtle; font.pixelSize: 16 }
                Switch { checked: ctl.dark; onToggled: ctl.dark = checked }
            }
        }

        // tabs
        RowLayout {
            spacing: 6
            TabButton2 { text: "Benchmark"; index: 0 }
            TabButton2 { text: "System";    index: 1 }
            TabButton2 { text: "History";   index: 2 }
            TabButton2 { text: "Settings";  index: 3 }
            Item { Layout.fillWidth: true }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: win.tab

            // ───────────────────────── Benchmark ──────────────────────────
            ColumnLayout {
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true
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
                    Layout.fillWidth: true
                    spacing: 8
                    Primary { text: "Submit ↑"; enabled: !ctl.running && ctl.has_results; onClicked: ctl.submit() }
                    Pill { text: "Share"; enabled: !ctl.running && (ctl.has_results || ctl.output.length > 0); onClicked: ctl.share() }
                    Pill { text: "Copy"; enabled: ctl.output.length > 0; onClicked: { results.selectAll(); results.copy(); results.deselect() } }
                    Pill { text: "Export .txt"; enabled: ctl.output.length > 0; onClicked: saveReport.open() }
                    Pill { text: "Export .json"; enabled: ctl.has_results; onClicked: saveJson.open() }
                    Item { Layout.fillWidth: true }
                    Pill { text: "Clear"; enabled: !ctl.running && ctl.output.length > 0; onClicked: ctl.clear() }
                }

                // share URL
                Card {
                    theme: theme
                    Layout.fillWidth: true
                    visible: ctl.share_url.length > 0
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: ctl.backend === "crux" ? "Result:" : "Paste:"; color: theme.subtle }
                        TextField {
                            Layout.fillWidth: true
                            readOnly: true; selectByMouse: true
                            text: ctl.share_url
                            color: theme.text
                            background: Rectangle { color: theme.surface2; radius: 6; border.color: theme.border }
                        }
                        Pill { text: "Copy URL"; onClicked: { urlField.text = ctl.share_url } }
                    }
                }
                TextEdit { id: urlField; visible: false }

                // status
                RowLayout {
                    spacing: 8
                    BusyIndicator { running: ctl.running; visible: ctl.running; implicitWidth: 20; implicitHeight: 20 }
                    Label { text: ctl.status; color: theme.subtle }
                }

                // results
                Card {
                    theme: theme
                    title: "Results"
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    ScrollView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        TextArea {
                            id: results
                            readOnly: true
                            wrapMode: TextEdit.NoWrap
                            font.family: theme.mono
                            font.pixelSize: 13
                            color: theme.text
                            selectByMouse: true
                            background: null
                            text: ctl.output.length > 0
                                ? ctl.output
                                : "Pick a suite above to begin.\n\nThe Full Benchmark streams each suite's results in as it\nfinishes, records the run to local history, and lets you\nShare, Submit, or Export the JSON."
                        }
                    }
                }
            }

            // ───────────────────────── System ─────────────────────────────
            ColumnLayout {
                spacing: 12
                GridLayout {
                    Layout.fillWidth: true
                    columns: 3
                    columnSpacing: 12
                    rowSpacing: 12
                    Repeater {
                        model: [
                            { k: "Processor", v: win.facts.cpu_model || "—" },
                            { k: "Logical cores", v: (win.facts.logical_cores !== undefined ? String(win.facts.logical_cores) : "—") },
                            { k: "Memory", v: (win.facts.ram_mib ? (win.facts.ram_mib/1024).toFixed(1) + " GiB" : "—") },
                            { k: "Kernel", v: win.facts.kernel || "—" },
                            { k: "OS", v: win.facts.os || "—" }
                        ]
                        delegate: Card {
                            theme: theme
                            Layout.fillWidth: true
                            Layout.minimumWidth: 220
                            Label { text: modelData.k; color: theme.subtle; font.pixelSize: 11; font.capitalization: Font.AllUppercase }
                            Label { text: modelData.v; color: theme.text; font.pixelSize: 16; font.bold: true; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                        }
                    }
                }
                Card {
                    theme: theme
                    title: "Deep report"
                    Layout.fillWidth: true
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: theme.subtle
                            text: "The full `crux info` report — cache hierarchy, every disk, thermals, batteries, per-interface IPs — opens in the Benchmark tab's pane."
                        }
                        Primary { text: "Open full report"; enabled: !ctl.running; onClicked: { win.tab = 0; ctl.run("info", durationSpin.value, runsSpin.value) } }
                    }
                }
                Item { Layout.fillHeight: true }
            }

            // ───────────────────────── History ────────────────────────────
            ColumnLayout {
                spacing: 12

                // analysis summary
                GridLayout {
                    Layout.fillWidth: true
                    columns: 4
                    columnSpacing: 12
                    rowSpacing: 12
                    Repeater {
                        model: [
                            { k: "Runs recorded", v: win.fnum(win.ana.count) },
                            { k: "Best CPU (MT)", v: win.fnum(win.ana.best_mt) },
                            { k: "Best CPU (ST)", v: win.fnum(win.ana.best_st) },
                            { k: "Best Triad", v: win.fnum(win.ana.best_triad, 1) + " GB/s" }
                        ]
                        delegate: Card {
                            theme: theme
                            Layout.fillWidth: true
                            Label { text: modelData.k; color: theme.subtle; font.pixelSize: 11; font.capitalization: Font.AllUppercase }
                            Label { text: modelData.v; color: theme.accent; font.pixelSize: 20; font.bold: true }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Pill { text: "Refresh"; onClicked: ctl.refresh_history() }
                    Item { Layout.fillWidth: true }
                    Label { text: "Compare:"; color: theme.subtle }
                    ComboBox {
                        id: cmpA
                        Layout.preferredWidth: 230
                        model: win.histList
                        textRole: "headline"
                    }
                    ComboBox {
                        id: cmpB
                        Layout.preferredWidth: 230
                        model: win.histList
                        textRole: "headline"
                    }
                    Primary {
                        text: "Compare"
                        enabled: win.histList.length >= 2
                        onClicked: {
                            var a = win.histList[cmpA.currentIndex];
                            var b = win.histList[cmpB.currentIndex];
                            if (a && b) ctl.compare_runs(a.id, b.id);
                        }
                    }
                }

                // history list + compare output side by side
                RowLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: 12

                    Card {
                        theme: theme
                        title: "Recorded runs"
                        Layout.preferredWidth: 1
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        ListView {
                            id: histView
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            model: win.histList
                            spacing: 6
                            delegate: Rectangle {
                                width: histView.width
                                height: 54
                                radius: 8
                                color: theme.surface2
                                border.color: theme.border
                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                    spacing: 2
                                    Label { text: modelData.cpu || "—"; color: theme.text; font.bold: true; elide: Text.ElideRight; Layout.fillWidth: true }
                                    Label {
                                        color: theme.subtle; font.pixelSize: 11
                                        text: modelData.when + "   ·   MT " + win.fnum(modelData.mt) + " / ST " + win.fnum(modelData.st)
                                    }
                                }
                            }
                            Label {
                                anchors.centerIn: parent
                                visible: win.histList.length === 0
                                text: "No runs yet.\nRun a benchmark to record one."
                                horizontalAlignment: Text.AlignHCenter
                                color: theme.subtle
                            }
                        }
                    }

                    Card {
                        theme: theme
                        title: "Comparison"
                        Layout.preferredWidth: 1
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        ScrollView {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            TextArea {
                                readOnly: true
                                wrapMode: TextEdit.NoWrap
                                font.family: theme.mono
                                font.pixelSize: 12
                                color: theme.text
                                background: null
                                text: ctl.compare_text.length > 0 ? ctl.compare_text
                                    : "Choose two runs above and press Compare.\nLarger-is-better deltas are computed per metric."
                            }
                        }
                    }
                }
            }

            // ───────────────────────── Settings ───────────────────────────
            ColumnLayout {
                spacing: 12
                Card {
                    theme: theme
                    title: "Appearance"
                    Layout.fillWidth: true
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: "Dark theme"; color: theme.text; Layout.fillWidth: true }
                        Switch { checked: ctl.dark; onToggled: ctl.dark = checked }
                    }
                }
                Card {
                    theme: theme
                    title: "Score server"
                    Layout.fillWidth: true
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: theme.subtle
                        text: "Submit sends results to the CRUCIBLE score server (default\nhttps://crux.mmzsigmond.me). Override the target by setting the\nCRUX_SERVER environment variable before launching. Share falls\nback to paste.rs if the server is unreachable."
                    }
                }
                Card {
                    theme: theme
                    title: "Danger zone"
                    Layout.fillWidth: true
                    RowLayout {
                        Layout.fillWidth: true
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            Label { text: "Uninstall CRUCIBLE"; color: theme.text; font.bold: true }
                            Label { text: "Removes binaries, man page, completions and the desktop entry."; color: theme.subtle; font.pixelSize: 11 }
                        }
                        Pill { text: "Uninstall…"; base: theme.bad; fg: "#ffffff"; enabled: !ctl.running; onClicked: confirmUninstall.open() }
                    }
                }
                Item { Layout.fillHeight: true }
            }
        }
    }

    // Show the headline facts immediately; history is loaded by Default::default.
    Component.onCompleted: ctl.refresh_history()
}
