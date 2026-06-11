// GPU render benchmark (GUI-only).
//
// Drives the Qt Quick scene graph — and therefore the GPU through whichever
// RHI backend is active (Vulkan, OpenGL, Metal, D3D, …) — with an adaptive
// load of animated composite sprites. Vsync caps the frame rate, so raw fps
// is a poor score on its own; instead the bench ramps the number of animated
// items until the GPU can no longer hold ~92% of the refresh rate, then holds
// that peak load for a fixed measurement window. Score = items × fps / 100, a
// throughput number that stays meaningful under vsync.
//
// Load profile (bench v2 — scores are NOT comparable with v1 runs):
//   · each item is a composite sprite: rotating gradient body, counter-
//     rotating gradient core, rotating ring, plus a render-thread scale
//     pulse — ~4 blended, antialiased scene-graph nodes per item
//   · a fixed stack of full-arena translucent gradient sheets rotates
//     underneath, adding constant fill-rate/overdraw cost every frame
//
// All animation uses Animator types (render thread, no per-frame JS), so the
// load is GPU/scene-graph-bound rather than main-thread-bound. Timing uses
// FrameAnimation (Qt 6.4+). No ShaderEffect — that would need precompiled
// .qsb shaders.
//
// Ramp robustness: instantiating hundreds of composite delegates causes a
// one-frame stall, so the first fps window after every item-count change is
// discarded (skipWindow), and measurement sampling starts only after a short
// settle period.
//
// Phases: 0 idle · 1 warm-up · 2 ramp · 3 measure · 4 done.
import QtQuick

Item {
    id: root

    property var pal

    property int phase: 0
    readonly property bool active: phase >= 1 && phase <= 3
    readonly property string phaseName:
        phase === 1 ? "warming up" :
        phase === 2 ? "ramping load" :
        phase === 3 ? "measuring" :
        phase === 4 ? "done" : "idle"

    // live state (updates while running)
    property int items: 0
    property real liveFps: 0
    property real progress: 0

    // results (valid once phase === 4)
    property int score: 0
    property real avgFps: 0
    property real low1Fps: 0
    property int peakItems: 0
    property string apiName: "—"

    // The window's active graphics API, probed shortly after startup (the
    // scene graph needs a frame or two before GraphicsInfo.api is valid).
    property string activeApi: "—"
    Timer {
        interval: 700
        repeat: true
        running: root.activeApi === "—" || root.activeApi === "Unknown"
        onTriggered: root.activeApi = root.apiString()
    }

    /// Emitted at the end of a completed run with the result JSON:
    /// {bench, score, items, fps, low1_fps, api, refresh_hz}
    signal finished(string resultJson)

    // tuning
    readonly property int benchVersion: 2
    readonly property int warmupItems: 48
    readonly property int startItems: 64
    readonly property int maxItems: 4096
    readonly property int overdrawSheets: 6
    readonly property real rampFactor: 1.3
    readonly property real warmupSecs: 1.2
    readonly property real windowSecs: 0.4
    readonly property real rampTimeoutSecs: 20
    readonly property real settleSecs: 0.35
    readonly property real measureSecs: 5
    readonly property real targetFraction: 0.92

    // internals
    property real tPhase: 0
    property real winTime: 0
    property int winFrames: 0
    property real targetFps: 0
    property bool skipWindow: false
    property var samples: []

    function start() {
        if (active)
            return
        samples = []
        score = 0; avgFps = 0; low1Fps = 0; peakItems = 0; apiName = "—"
        liveFps = 0; progress = 0
        tPhase = 0; winTime = 0; winFrames = 0
        skipWindow = false
        items = warmupItems
        phase = 1
    }

    function stop() {
        if (!active)
            return
        phase = 0
        items = 0
        progress = 0
    }

    function refreshHz() {
        return Screen.refreshRate > 0 ? Screen.refreshRate : 60
    }

    FrameAnimation {
        running: root.active
        // clamp stalls (window drag, etc.) so one bad frame can't skew a phase
        onTriggered: root.tick(Math.min(frameTime, 0.25))
    }

    function tick(dt) {
        tPhase += dt
        winTime += dt
        winFrames += 1
        // skip the settle period so the backoff/creation stall stays out of
        // the measured samples
        if (phase === 3 && tPhase >= settleSecs)
            samples.push(dt)

        if (winTime >= windowSecs) {
            liveFps = winFrames / winTime
            winTime = 0
            winFrames = 0
            if (phase === 2) {
                if (skipWindow)
                    skipWindow = false
                else
                    rampStep()
            }
        }

        if (phase === 1) {
            progress = 0.15 * Math.min(1, tPhase / warmupSecs)
            if (tPhase >= warmupSecs) {
                var base = liveFps > 0 ? liveFps : refreshHz()
                targetFps = targetFraction * Math.min(base, refreshHz())
                items = startItems
                skipWindow = true
                phase = 2
                tPhase = 0; winTime = 0; winFrames = 0
            }
        } else if (phase === 2) {
            progress = 0.15 + 0.35 * Math.min(1, tPhase / rampTimeoutSecs)
        } else if (phase === 3) {
            progress = 0.5 + 0.5 * Math.min(1, tPhase / (measureSecs + settleSecs))
            if (tPhase >= measureSecs + settleSecs)
                finish()
        }
    }

    // One completed (non-skipped) window during the ramp: grow while fps
    // holds, back off one step and start measuring the moment it drops.
    function rampStep() {
        if (liveFps >= targetFps && items < maxItems && tPhase < rampTimeoutSecs) {
            items = Math.min(maxItems, Math.ceil(items * rampFactor))
            // discard the window polluted by delegate creation
            skipWindow = true
        } else {
            if (liveFps < targetFps && items > startItems)
                items = Math.max(startItems, Math.round(items / rampFactor))
            peakItems = items
            samples = []
            phase = 3
            tPhase = 0; winTime = 0; winFrames = 0
        }
    }

    function finish() {
        var n = samples.length
        var sum = 0
        for (var i = 0; i < n; ++i)
            sum += samples[i]
        avgFps = (n > 0 && sum > 0) ? n / sum : 0

        // 1% lows: mean fps over the worst (longest) 1% of frame times
        var sorted = samples.slice().sort(function(a, b) { return b - a })
        var k = Math.max(1, Math.floor(n / 100))
        var wsum = 0
        for (var j = 0; j < k && j < sorted.length; ++j)
            wsum += sorted[j]
        low1Fps = wsum > 0 ? k / wsum : 0

        apiName = apiString()
        score = Math.round(peakItems * avgFps / 100)
        items = 0
        progress = 1
        phase = 4
        finished(JSON.stringify({
            bench: benchVersion,
            score: score,
            items: peakItems,
            fps: Math.round(avgFps * 10) / 10,
            low1_fps: Math.round(low1Fps * 10) / 10,
            api: apiName,
            refresh_hz: Math.round(refreshHz() * 10) / 10
        }))
    }

    function apiString() {
        switch (root.GraphicsInfo.api) {
        case GraphicsInfo.OpenGL:     return "OpenGL"
        case GraphicsInfo.Vulkan:     return "Vulkan"
        case GraphicsInfo.Metal:      return "Metal"
        case GraphicsInfo.Direct3D11: return "Direct3D 11"
        case GraphicsInfo.Direct3D12: return "Direct3D 12"
        case GraphicsInfo.Software:   return "Software"
        default:                      return "Unknown"
        }
    }

    // Deterministic pseudo-random in [0,1) so every run lays out the same scene.
    function frand(i, salt) {
        var x = Math.sin(i * 127.1 + salt * 311.7) * 43758.5453
        return x - Math.floor(x)
    }

    Rectangle {
        id: arena
        anchors.fill: parent
        radius: 8
        color: root.pal ? root.pal.surface2 : "#1d212b"
        border.color: root.pal ? root.pal.border : "#262b36"
        border.width: 1
        clip: true

        // Constant fill-rate load: full-arena blended gradient sheets that
        // repaint every pixel each frame, stacked under the sprites.
        Repeater {
            model: root.active ? root.overdrawSheets : 0
            delegate: Rectangle {
                id: sheet
                required property int index
                anchors.centerIn: parent
                width: Math.max(arena.width, arena.height) * 1.6
                height: width
                opacity: 0.07
                gradient: Gradient {
                    GradientStop { position: 0.0; color: Qt.hsla(root.frand(sheet.index, 20), 0.8, 0.55, 1) }
                    GradientStop { position: 0.5; color: Qt.hsla(root.frand(sheet.index, 21), 0.8, 0.4, 1) }
                    GradientStop { position: 1.0; color: Qt.hsla(root.frand(sheet.index, 22), 0.8, 0.25, 1) }
                }
                RotationAnimator on rotation {
                    from: sheet.index * 60
                    to: sheet.index * 60 + (sheet.index % 2 === 0 ? 360 : -360)
                    duration: 6000 + sheet.index * 1500
                    loops: Animation.Infinite
                    running: root.active
                }
            }
        }

        Repeater {
            model: root.items
            delegate: Item {
                id: sprite
                required property int index
                readonly property real size: 90 + root.frand(index, 2) * 220
                width: size
                height: size
                x: root.frand(index, 3) * Math.max(1, arena.width - size)
                y: root.frand(index, 4) * Math.max(1, arena.height - size)

                // rotating gradient body
                Rectangle {
                    anchors.fill: parent
                    radius: sprite.size / 4
                    opacity: 0.5
                    antialiasing: true
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: Qt.hsla(root.frand(sprite.index, 5), 0.75, 0.55, 1) }
                        GradientStop { position: 0.5; color: Qt.hsla(root.frand(sprite.index, 9), 0.8, 0.45, 1) }
                        GradientStop { position: 1.0; color: Qt.hsla(root.frand(sprite.index, 6), 0.75, 0.35, 1) }
                    }
                    RotationAnimator on rotation {
                        from: root.frand(sprite.index, 7) * 360
                        to: root.frand(sprite.index, 7) * 360 + 360
                        duration: 1200 + Math.round(root.frand(sprite.index, 8) * 2400)
                        loops: Animation.Infinite
                        running: root.active
                    }
                }

                // counter-rotating gradient core
                Rectangle {
                    anchors.centerIn: parent
                    width: sprite.size * 0.58
                    height: width
                    radius: width / 3
                    opacity: 0.65
                    antialiasing: true
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: Qt.hsla(root.frand(sprite.index, 10), 0.85, 0.6, 1) }
                        GradientStop { position: 1.0; color: Qt.hsla(root.frand(sprite.index, 11), 0.85, 0.35, 1) }
                    }
                    RotationAnimator on rotation {
                        from: root.frand(sprite.index, 12) * 360
                        to: root.frand(sprite.index, 12) * 360 - 360
                        duration: 900 + Math.round(root.frand(sprite.index, 13) * 1800)
                        loops: Animation.Infinite
                        running: root.active
                    }
                }

                // rotating ring outline
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: -sprite.size * 0.08
                    radius: width / 4
                    color: "transparent"
                    border.width: 3
                    border.color: Qt.hsla(root.frand(sprite.index, 14), 0.7, 0.6, 0.8)
                    antialiasing: true
                    RotationAnimator on rotation {
                        from: root.frand(sprite.index, 15) * 360
                        to: root.frand(sprite.index, 15) * 360 + 360
                        duration: 2000 + Math.round(root.frand(sprite.index, 16) * 3000)
                        loops: Animation.Infinite
                        running: root.active
                    }
                }

                // pulsing fill area (animators stay on the render thread)
                SequentialAnimation on scale {
                    running: root.active
                    loops: Animation.Infinite
                    ScaleAnimator {
                        from: 0.75
                        to: 1.2
                        duration: 1000 + Math.round(root.frand(sprite.index, 17) * 1600)
                    }
                    ScaleAnimator {
                        from: 1.2
                        to: 0.75
                        duration: 1000 + Math.round(root.frand(sprite.index, 17) * 1600)
                    }
                }
            }
        }

        Column {
            anchors.centerIn: parent
            visible: !root.active
            spacing: 6

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: root.phase === 4
                color: root.pal ? root.pal.accent : "#e0552b"
                font.pixelSize: 54
                font.bold: true
                text: root.score
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                color: root.pal ? root.pal.subtle : "#9aa1b1"
                font.pixelSize: 14
                horizontalAlignment: Text.AlignHCenter
                text: root.phase === 4
                    ? root.peakItems + " items sustained at " + root.avgFps.toFixed(1)
                      + " fps · " + root.apiName
                    : "The arena fills with layered animated sprites while the\nbenchmark ramps the load. Keep this window visible."
            }
        }
    }
}
