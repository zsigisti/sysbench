# Packaging

CRUCIBLE is meant to be **built on the host it will run on**, so the benchmark
reflects that machine's CPU. All three packaging paths below compile from source
locally rather than shipping a prebuilt binary.

> ⚠️ **Native binaries are not portable.** When built with
> `-C target-cpu=native`, the binary uses every instruction the build host
> supports and may crash with `SIGILL` on a different/older CPU. Build the
> package **on the target machine** (or drop `target-cpu=native` for a portable,
> baseline build — at the cost of benchmark realism). Do not upload a
> native-built `.deb`/`.rpm`/`.pkg.tar.zst` to a shared repository.

## Why this works per-format

| Format | Build location | Native by default? |
|--------|----------------|--------------------|
| **AUR** | `makepkg` compiles on the user's machine | ✅ yes — that's how AUR works |
| **deb** | `cargo deb` run on your host | ✅ when you build it locally |
| **rpm** | `cargo generate-rpm` run on your host | ✅ when you build it locally |

## Arch / AUR

The `PKGBUILD` lives in [`packaging/aur/PKGBUILD`](../packaging/aur/PKGBUILD). It
builds `crux` with `-C target-cpu=native`, installs the binary, and symlinks
`sysinfo → crux`.

```sh
cd packaging/aur
# point source= at a real release tag and set sha256sums, then:
makepkg -si
```

For a personal install you can also just run the repo's `install.sh`.

To publish to the AUR, push the `PKGBUILD` (plus a generated `.SRCINFO`) to an
AUR git repo named `crucible`:

```sh
makepkg --printsrcinfo > .SRCINFO
```

## Man page & shell completions

`crux` generates its own man page and completions from the real CLI:

```sh
crux man                  # roff man page → stdout
crux completions bash     # also: zsh | fish | elvish | powershell
```

The AUR `PKGBUILD` calls these directly in `package()`. For deb/rpm, the asset
lists reference `target/assets/`, so run the generator first:

```sh
packaging/gen-assets.sh   # writes target/assets/{crux.1,completions/*}
```

`install.sh` installs the man page and bash/zsh/fish completions automatically
(to system dirs as root, `~/.local`/`~/.config` otherwise).

## Debian / Ubuntu (`cargo deb`)

```sh
cargo install cargo-deb
packaging/gen-assets.sh   # man page + completions (see above)
cargo deb                 # produces target/debian/crucible_<ver>_<arch>.deb
sudo dpkg -i target/debian/crucible_*.deb
```

The deb metadata is in `Cargo.toml` under `[package.metadata.deb]`. The
maintainer scripts in [`packaging/deb/`](../packaging/deb/) create and remove the
`sysinfo` alias on install/removal.

For a host-native deb, build with:

```sh
RUSTFLAGS="-C target-cpu=native" cargo deb
```

## Fedora / RHEL / openSUSE (`cargo generate-rpm`)

```sh
cargo install cargo-generate-rpm
RUSTFLAGS="-C target-cpu=native" cargo build --release
packaging/gen-assets.sh  # man page + completions
cargo generate-rpm  # produces target/generate-rpm/crucible-<ver>.<arch>.rpm
sudo rpm -i target/generate-rpm/crucible-*.rpm
```

The rpm metadata is in `Cargo.toml` under `[package.metadata.generate-rpm]`,
including `post_install`/`post_uninstall` scripts that manage the `sysinfo`
alias.

## Portable (distributable) builds

If you genuinely need one artifact that runs on many machines, drop the native
flag and target a baseline microarchitecture, e.g.:

```sh
RUSTFLAGS="-C target-cpu=x86-64-v2" cargo build --release
```

The benchmark will then under-report what newer CPUs can do, but the binary
won't fault on older ones. CRUCIBLE's whole premise is host-native builds, so
prefer building on the target whenever you can.
