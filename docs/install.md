# Install

## Manjaro / Arch

Download the prebuilt package from the latest release:

```sh
TAG=v0.0.5
PKG="ticonnect-${TAG#v}-1-x86_64.pkg.tar.zst"
curl -LO "https://github.com/TICO-CO/TiConnect/releases/download/$TAG/$PKG"
sudo pacman -U "$PKG"
```

The package installs `ticonnect` to `/usr/bin`, the app-menu entry and icon,
systemd user units, and XDG autostart entries for the daemon and overlay. The
daemon and overlay start at the next login. Start them now with:

```sh
systemctl --user start ticonnectd.service ticonnect-overlay.service
ticonnect ui
```

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
daemon and overlay systemd user services. Package-only install without services:

```sh
nix profile install github:TICO-CO/TiConnect#ticonnect
```

Uninstall by removing the module/input and rebuilding, or
`nix profile remove ticonnect`.

## Flatpak

```sh
flatpak-builder --user --install --force-clean build packaging/flatpak/org.ticonnect.TiConnect.yml
flatpak run org.ticonnect.TiConnect
```

The manifest builds the unified `ticonnect` executable from one source
checkout. It requests network access, the session bus, Wayland with an X11
fallback, and read/write access only to the TiConnect config and data
directories.

Portal-backed capture and input, notifications, autostart, and the status
notifier must be provided by the host desktop. Portals cannot be bypassed by the
sandbox.

## AppImage

The release AppImage bundles GTK4, libadwaita, gtk4-layer-shell, ALSA, GLib
schemas, and gdk-pixbuf loaders. The host still provides the kernel, glibc
(2.39+), GPU drivers, display server, portals, and audio devices.

```sh
TAG=v0.0.5
curl -LO "https://github.com/TICO-CO/TiConnect/releases/download/$TAG/ticonnect-$TAG-x86_64.AppImage"
chmod +x "ticonnect-$TAG-x86_64.AppImage"
"./ticonnect-$TAG-x86_64.AppImage" --help
"./ticonnect-$TAG-x86_64.AppImage" daemon
"./ticonnect-$TAG-x86_64.AppImage" cli status
"./ticonnect-$TAG-x86_64.AppImage" ui
```

Without FUSE, or on NixOS, use the extract-and-run flag:

```sh
"./ticonnect-$TAG-x86_64.AppImage" --appimage-extract-and-run --help
```

On NixOS, `appimage-run ./ticonnect-v0.0.5-x86_64.AppImage` also works. On
Manjaro install `fuse2` for direct execution, or use
`--appimage-extract-and-run`. Pairing and firewall rules are covered in the
README.

## From source

Dependencies on Debian/Ubuntu:

```sh
sudo apt install build-essential cmake pkg-config libgtk-4-dev libadwaita-1-dev \
  libasound2-dev libwayland-dev wayland-protocols libxkbcommon-dev \
  libx11-dev libxi-dev libxtst-dev
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
systemctl --user daemon-reload
systemctl --user enable --now ticonnectd.service
```

`make install-user` installs `ticonnect` to `$(PREFIX)/bin` and the units point
at `ticonnect daemon`, `ticonnect overlay`, and `ticonnect ui`.

The `systemd` unit starts the daemon at login. The overlay and UI units are
optional:

```sh
systemctl --user enable --now ticonnect-overlay.service
systemctl --user enable --now ticonnect-ui.service
```

Autostart entries under `packaging/autostart/` install to
`~/.config/autostart/`. Set `X-GNOME-Autostart-enabled=false` to disable one.

## Data locations

- Config: `$XDG_CONFIG_HOME/ticonnect/config.toml`
- Identity: `$XDG_CONFIG_HOME/ticonnect/identity.key` (mode `0600`)
- Data and history: `$XDG_DATA_HOME/ticonnect/ticonnect.db`
- Attachments: `$XDG_DATA_HOME/ticonnect/attachments/`

## Uninstall

```sh
make uninstall-user
rm -rf ~/.config/ticonnect ~/.local/share/ticonnect
sudo pacman -Rns ticonnect
flatpak uninstall org.ticonnect.TiConnect
```
