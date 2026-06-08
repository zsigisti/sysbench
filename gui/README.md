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

- Buttons to run the **Full Benchmark** or any individual suite (CPU, Memory,
  Network, Storage), plus **System Info**.
- Runs the work on a background thread (the UI stays responsive) and streams the
  status back via Qt signals.
- Shows the same human-readable report as `crux`, in a monospace results pane.

## Layout

- `src/controller.rs` — the `#[cxx_qt::bridge]` `Controller` QObject: holds
  `status` / `running` / `output` properties and a `run(kind)` invokable that
  spawns the engine on a worker thread and marshals results back onto the Qt
  event loop with `qt_thread().queue(...)`.
- `src/main.rs` — boots `QGuiApplication` + `QQmlApplicationEngine`.
- `qml/main.qml` — the UI.
- `build.rs` — `cxx-qt-build` wires the QML module + Rust bridge together.

## Desktop integration

`packaging/crux-gui.desktop` + `assets/logo.svg` (installed as the `crucible`
icon) provide an application-menu entry. These get wired into the OS packages
when packaging is set up — see [../docs/packaging.md](../docs/packaging.md).
