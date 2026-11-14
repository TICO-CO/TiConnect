# Install

## Flatpak

```sh
flatpak-builder --user --install --force-clean build packaging/flatpak/org.ticonnect.TiConnect.yml
flatpak run org.ticonnect.TiConnect
```

The manifest builds `ticonnectd`, `ticonnect-ui`, `ticonnect-ovl`, and
`ticonnect-cli` from one source checkout. It requests network access, the
session bus, Wayland with an X11 fallback, and read/write access only to the
TiConnect config and data directories.

Portal-backed capture and input, notifications, autostart, and the status
notifier must be provided by the host desktop. Portals cannot be bypassed by the
sandbox.

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
cargo build --release --workspace --bins
make install-user
systemctl --user daemon-reload
systemctl --user enable --now ticonnectd.service
```

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
flatpak uninstall org.ticonnect.TiConnect
```
