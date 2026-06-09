// Central palette. Instantiated once in main.qml and threaded through the UI so
// a single `dark` flag re-colours everything.
import QtQuick

QtObject {
    property bool dark: true

    readonly property color bg:       dark ? "#0e1014" : "#f4f5f7"
    readonly property color surface:  dark ? "#191c23" : "#ffffff"
    readonly property color surface2: dark ? "#222631" : "#eceef2"
    readonly property color border:   dark ? "#2c313d" : "#d9dce3"
    readonly property color text:     dark ? "#e7e9ef" : "#1a1c21"
    readonly property color subtle:   dark ? "#9aa1b1" : "#5d636f"

    // CRUCIBLE ember accents
    readonly property color accent:   "#e0552b"
    readonly property color accent2:  "#f0a020"
    readonly property color good:     dark ? "#46c95a" : "#1f9d38"
    readonly property color bad:      dark ? "#f0705a" : "#d23b27"

    readonly property string mono: "monospace"
    readonly property int radius: 10
}
