# crux-gui — CRUCIBLE Qt GUI

A Qt 6 / QML desktop front-end for the CRUCIBLE benchmark engine. It links the
same `crucible` library the `crux` CLI uses, so every number it shows is
produced by identical measurement code.

The CLI (`crux`) remains fully standalone — the GUI is an **optional** extra in
its own crate and is never required to use CRUCIBLE.

## Install & run

Requires Qt 6 (Quick/QML) and a C++ toolchain at build time.

```sh
# install via the script (builds + adds an app-menu entry + icon)
./install.sh --gui          # or: ./install.sh --all  (CLI + GUI)
crux-gui

# …or build directly from a clone
cargo build -p crucible-gui --release
./target/release/crux-gui
```

Build dependencies:

| Distro | Packages |
|--------|----------|
| Arch | `qt6-base qt6-declarative gcc` |
| Debian/Ubuntu | `qt6-base-dev qt6-declarative-dev build-essential` |
| Fedora | `qt6-qtbase-devel qt6-qtdeclarative-devel gcc-c++` |

> A bare `cargo build` at the repo root only builds the CLI (the workspace's
> `default-members`), so CLI users never need Qt installed. The GUI is built
> explicitly with `-p crucible-gui`.

## What it does

A tabbed, themeable desktop app over the same engine as the CLI. Everything it
measures is produced by identical code, so the numbers match `crux` exactly.

**Benchmark tab**
- Run the **Full Benchmark** or any individual suite (CPU, Memory, Network,
  Storage), plus **System Info**. **Duration** and **Runs** feed the CPU suites.
- The Full Benchmark **streams** each suite in as it finishes, then merges the
  results into one record and **records it to local history**.
- **Submit** sends the run to the CRUCIBLE score server; **Share** uses the
  server with a paste.rs fallback; **Copy** copies the report; **Export .txt**
  and **Export .json** save it to disk; **Clear** resets the pane.

**Render tab** (GUI-only — the CLI has no equivalent)
- A **GPU render benchmark** driving the Qt Quick scene graph through whatever
  RHI backend is active (Vulkan or OpenGL on Linux). It warms up, **ramps** the
  number of render-thread-animated gradient quads until the GPU can no longer
  hold ~92% of the display refresh rate, then **measures** a 5-second sustained
  window. Score = items × fps / 100 (throughput — meaningful even under vsync),
  plus avg FPS, 1% lows, sustained item count, and the graphics API used.
- The result is merged into the run JSON as a `"render"` section, so **Submit /
  Share / Export** carry it and the leaderboard can rank it; it is also recorded
  to local history and shows up in Compare.
- A **backend toggle** (Auto / OpenGL / Vulkan) — Qt fixes the RHI backend at
  startup, so the choice is persisted to `~/.config/crucible/gui.json`, applied
  as `QSG_RHI_BACKEND` at launch, and a one-click **Restart to apply** relaunches
  the app. The currently active API is shown live (probed via `GraphicsInfo`).

**System tab** — the machine's headline facts (CPU, cores, RAM, kernel, OS,
anonymized machine ID) as clean key/value cards, plus a button to open the full
deep report.

**History tab** — local run analysis with no server required: a summary
(runs recorded, best MT/ST/Triad/Render), the list of past runs, and a
**Compare** panel that diffs any two recorded runs metric-by-metric.

**Settings tab** — dark/light theme toggle (persisted), the renderer backend
toggle, score-server notes (`CRUX_SERVER`), and an **Uninstall** action (with
optional "delete local history") that removes the installed binaries, man page,
completions and desktop entry.

All work runs on a background thread (the UI stays responsive) and is marshalled
back onto the Qt event loop via `qt_thread().queue(...)`.

The theme toggle (top-right or Settings) recolours the whole UI; history lives
in `~/.local/share/crucible/history`. You can also uninstall from the CLI with
`crux uninstall` or `./install.sh --uninstall`.

## Layout

- `src/controller.rs` — the `#[cxx_qt::bridge]` `Controller` QObject. Properties
  (`status`, `running`, `output`, `share_url`, `backend`, `dark`, `has_results`,
  `sys_facts`, `history`, `analysis`, `compare_text`, `last_json`,
  `render_result`, `render_backend`, `boot_backend`, `app_version`) and
  invokables (`run`, `share`, `submit`, `clear`, `export_report`, `export_json`,
  `refresh_history`, `compare_runs`, `uninstall`, `record_render`,
  `choose_render_backend`, `set_dark_pref`, `restart`). Benchmarks run on worker
  threads and marshal back via `qt_thread().queue(...)`. JSON string properties
  (history/analysis/sys_facts) are parsed in QML with `JSON.parse`.
- `src/main.rs` — boots `QGuiApplication` + `QQmlApplicationEngine`. Forces the
  **Basic** Quick Controls style (`QT_QUICK_CONTROLS_STYLE`) so a distro style
  like KDE Breeze can't override the custom theming, and applies the persisted
  renderer choice as `QSG_RHI_BACKEND` before Qt starts.
- `src/prefs.rs` — tiny persisted preferences (`~/.config/crucible/gui.json`):
  theme + renderer backend, written atomically (temp file + rename).
- `qml/main.qml` — the UI: a left nav rail (Benchmark / Render / System /
  History / Settings) + content. The palette lives as `readonly property color …`
  on the **root** `ApplicationWindow` id (driven by `ctl.dark`) so it resolves
  inside `Repeater`/`ListView` delegates too.
- `qml/RenderBench.qml` — the self-contained GPU benchmark: a `FrameAnimation`
  phase machine (warm-up → ramp → measure) over `RotationAnimator`-driven quads
  (render thread, no per-frame JS), reporting `{score, items, fps, low1_fps,
  api, refresh_hz}` via `finished()` → `ctl.record_render()`.
- `qml/StatCard.qml` — a pure key/value tile (plain props only) used inside grid
  delegates; `qml/Panel.qml` — a titled surface used outside delegates.
- `assets/logo.svg` is copied to `qml/logo.svg` by `build.rs` and bundled into
  the module via `qrc_resources`, so the UI shows the real logo.
- `build.rs` — `cxx-qt-build` wires the QML module + Rust bridge; QML files are
  registered in `qml_files([...])` and the logo in `qrc_resources([...])`.

## Desktop integration

`packaging/crux-gui.desktop` + `assets/logo.svg` (installed as the `crucible`
icon) provide an application-menu entry. These get wired into the OS packages
when packaging is set up — see [../docs/packaging.md](../docs/packaging.md).
