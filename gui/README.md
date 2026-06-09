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

**System tab** — the machine's headline facts (CPU, cores, RAM, kernel, OS) as
clean key/value cards, plus a button to open the full deep report.

**History tab** — local run analysis with no server required: a summary
(runs recorded, best MT/ST/Triad), the list of past runs, and a **Compare**
panel that diffs any two recorded runs metric-by-metric.

**Settings tab** — dark/light theme toggle, score-server notes (`CRUX_SERVER`),
and an **Uninstall** action (with optional "delete local history") that removes
the installed binaries, man page, completions and desktop entry.

All work runs on a background thread (the UI stays responsive) and is marshalled
back onto the Qt event loop via `qt_thread().queue(...)`.

The theme toggle (top-right or Settings) recolours the whole UI; history lives
in `~/.local/share/crucible/history`. You can also uninstall from the CLI with
`crux uninstall` or `./install.sh --uninstall`.

## Layout

- `src/controller.rs` — the `#[cxx_qt::bridge]` `Controller` QObject. Properties
  (`status`, `running`, `output`, `share_url`, `backend`, `dark`, `has_results`,
  `sys_facts`, `history`, `analysis`, `compare_text`, `last_json`) and invokables
  (`run`, `share`, `submit`, `clear`, `export_report`, `export_json`,
  `refresh_history`, `compare_runs`, `uninstall`). Benchmarks run on worker
  threads and marshal back via `qt_thread().queue(...)`. JSON string properties
  (history/analysis/sys_facts) are parsed in QML with `JSON.parse`.
- `src/main.rs` — boots `QGuiApplication` + `QQmlApplicationEngine`. Forces the
  **Basic** Quick Controls style (`QT_QUICK_CONTROLS_STYLE`) so a distro style
  like KDE Breeze can't override the custom theming.
- `qml/main.qml` — the UI: a left nav rail (Benchmark / System / History /
  Settings) + content. The palette lives as `readonly property color …` on the
  **root** `ApplicationWindow` id (driven by `ctl.dark`) so it resolves inside
  `Repeater`/`ListView` delegates too.
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
