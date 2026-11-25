# Install

## AppImage

Download `ticonnect-<tag>-x86_64.AppImage` from the latest release and install
the per-user app integration:

```sh
chmod +x ticonnect-v0.1.0-x86_64.AppImage
./ticonnect-v0.1.0-x86_64.AppImage --install-user
# launch TiConnect from the app menu
```

The installer extracts the AppImage once to
`$XDG_DATA_HOME/ticonnect/app/`, writes a wrapper to `~/.local/bin/ticonnect`,
adds the desktop entry, icon, metainfo, systemd user units, and autostart
entries for the daemon, overlay, and UI, and enables the daemon and overlay
services when a user systemd session is available. It records every installed
path in `$XDG_DATA_HOME/ticonnect/install-manifest.txt`. Autostart entries remain
the fallback.

Ensure `~/.local/bin` is on your session `PATH`. If
`~/.local/bin/ticonnect` already exists and was not created by TiConnect, the
installer refuses to overwrite it unless you pass `--force`.

Without FUSE, or on NixOS, use the extract-and-run flag:

```sh
./ticonnect-v0.1.0-x86_64.AppImage --appimage-extract-and-run --install-user
```

On NixOS, `appimage-run ./ticonnect-v0.1.0-x86_64.AppImage --install-user` also
works.

## Arch / Manjaro

```sh
TAG=v0.1.0
PKG="ticonnect-${TAG#v}-1-x86_64.pkg.tar.zst"
curl -LO "https://github.com/TICO-CO/TiConnect/releases/download/$TAG/$PKG"
sudo pacman -U "$PKG"
```

The package installs `ticonnect` to `/usr/bin`, the app-menu entry and icon,
systemd user units, and XDG autostart entries for the daemon and overlay. The
daemon and overlay start at the next login.

Uninstall:

```sh
sudo pacman -Rns ticonnect
```

## NixOS

Add the flake input and module:

```nix
{
  inputs.ticonnect.url = "github:TICO-CO/TiConnect";

  # inside your NixOS configuration module
  imports = [ inputs.ticonnect.nixosModules.default ];
  services.ticonnect.enable = true;
}
```

Then:

```sh
sudo nixos-rebuild switch --flake /etc/nixos
```

The module installs the package, adds the app-menu entry, and enables the
daemon and overlay systemd user services. Set `services.ticonnect.openFirewall =
true` to allow the default UDP ports `5353`, `45870`, and `45871`. Package-only
install without services:

```sh
nix profile install github:TICO-CO/TiConnect#ticonnect
```

Uninstall by removing the module/input and rebuilding, or
`nix profile remove ticonnect`.

## Command line

Install the app integration first when using the AppImage. Then use `ticonnect`
directly:

```sh
ticonnect status
ticonnect id show
ticonnect send <user-id> "hello"
ticonnect users list
ticonnect settings list
ticonnect doctor
```

The command line starts the daemon when it is absent. `ticonnect doctor` checks
local prerequisites without starting the daemon. `ticonnect --help` lists
the user-facing surface; `ticonnect --version` prints the binary version.
`ticonnect version` is an alias. `ticonnect cli ...` remains a hidden
compatibility alias.

## Uninstall and purge

For an AppImage per-user install:

```sh
ticonnect --uninstall-user
```

This removes the wrapper, app payload, desktop entry, icon, metainfo, units, and
autostart entries. It keeps `$XDG_CONFIG_HOME/ticonnect`,
`$XDG_DATA_HOME/ticonnect`, and `$XDG_STATE_HOME/ticonnect`, including identity,
database, attachments, and logs.

To remove all user data as well:

```sh
ticonnect --uninstall-user --purge
```

The purge asks for confirmation. In non-interactive scripts set
`TICONNECT_INSTALL_ASSUME_YES=1`.

For a source install, use `make uninstall-user` instead.

## Troubleshooting

```sh
ticonnect status
systemctl --user status ticonnectd
journalctl --user -u ticonnectd
```

Daemon logs also go to `$XDG_STATE_HOME/ticonnect/daemon.log`, usually
`~/.local/state/ticonnect/daemon.log`.

If the app menu entry does not launch, verify `~/.local/bin` is in your session
`PATH` and run `ticonnect` from a terminal to see the error.

If peers are not discovered or cannot connect, check LAN access:

```sh
ticonnect network status
ticonnect network check
```

The daemon uses mDNS (`5353/udp`), UDP broadcast discovery (`45870/udp`), and
QUIC transport (`45871/udp` by default). On systems with a firewall, allow those
UDP ports on the LAN. Examples:

```sh
sudo ufw allow 5353/udp
sudo ufw allow 45870/udp
sudo ufw allow 45871/udp
```

```sh
sudo firewall-cmd --add-port=5353/udp --add-port=45870/udp --add-port=45871/udp --permanent
sudo firewall-cmd --reload
```

`no_peers` means no peer replied; it does not prove the firewall is blocking
traffic. `peers_unreachable` means discovery worked but the transport port did
not. Check client isolation, VPNs, and the configured `transport.port`.

## Flatpak

Flatpak is the recommended tester path. Download
`org.ticonnect.TiConnect-<tag>.flatpak` from the latest release, then install
it:

```sh
flatpak install --user ./org.ticonnect.TiConnect-v0.1.0.flatpak
flatpak run org.ticonnect.TiConnect doctor
flatpak run org.ticonnect.TiConnect
```

The bundle includes the media codec extension and `yt-dlp`. Portal-backed
capture and input, notifications, tray integration, audio devices, and LAN UDP
access must still be provided by the host desktop. See
`docs/tester-quickstart.md` for the full two-machine checklist.

Developers can build the bundle locally:

```sh
scripts/package-flatpak.sh org.ticonnect.TiConnect packaging/flatpak/org.ticonnect.TiConnect.yml v0.1.0 dist
```

## From source

Dependencies on Debian/Ubuntu:

```sh
sudo apt install build-essential cmake pkg-config libgtk-4-dev   libwebkitgtk-6.0-dev libasound2-dev libwayland-dev wayland-protocols   libxkbcommon-dev libx11-dev libxi-dev libxtst-dev
```

`gtk4-layer-shell` is required by the overlay. Build it from source when it is
not packaged:

```sh
git clone --depth 1 --branch v1.0.4 https://github.com/wmww/gtk4-layer-shell.git
meson setup gtk4-layer-shell/build gtk4-layer-shell -Dintrospection=false -Dvapi=false
sudo meson install -C gtk4-layer-shell/build
```

Build and install as a user:

```sh
cargo build --release --bin ticonnect
make install-user
```

`make install-user` installs `ticonnect` to `$(PREFIX)/bin`, the app-menu entry,
icon, metainfo, systemd units, and autostart entries. It does not start or
enable services; autostart entries start the daemon, overlay, and UI, or your
desktop session starts them.

## Data locations

- Config: `$XDG_CONFIG_HOME/ticonnect/config.toml`
- Identity: `$XDG_CONFIG_HOME/ticonnect/identity.key` (mode `0600`)
- Data and history: `$XDG_DATA_HOME/ticonnect/ticonnect.db`
- Attachments: `$XDG_DATA_HOME/ticonnect/attachments/`
- App payload: `$XDG_DATA_HOME/ticonnect/app/`
- Installer manifest: `$XDG_DATA_HOME/ticonnect/install-manifest.txt`
- Daemon log: `$XDG_STATE_HOME/ticonnect/daemon.log`
