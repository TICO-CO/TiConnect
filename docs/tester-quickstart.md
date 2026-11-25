# Tester quickstart

This is the fastest supported path for testing TiConnect on a normal Linux
desktop. It uses the release Flatpak bundle because that bundle carries the
application, desktop libraries, media codec extension, and `yt-dlp`.

## What you need

- A graphical Linux desktop with D-Bus, Wayland or X11, and audio.
- Flatpak installed and Flathub available.
- Two machines or VMs on the same LAN for messaging, calls, files, and remote
  desktop.
- UDP ports `5353`, `45870`, and `45871` reachable between the test machines.

The sandbox cannot install host packages. For file-attachment testing, copy the
test file to your `Downloads` folder first; the tester build can read that
directory but not the rest of your home directory.

## Install the tester build

Download the latest release bundle:

```sh
flatpak install --user ./org.ticonnect.TiConnect-<tag>.flatpak
```

Replace `<tag>` with the release tag, for example `v0.1.0`.

Check the installation and local prerequisites:

```sh
flatpak run org.ticonnect.TiConnect doctor
```

For a bug report, save machine-readable output:

```sh
flatpak run org.ticonnect.TiConnect doctor --json > ticonnect-doctor.json
```

A missing display, D-Bus address, or unreadable config directory is a failure.
Missing audio, `yt-dlp`, or GStreamer plugins are warnings: messaging can still
work, while calls or media playback may not.

## Run the app

Launch TiConnect from the application menu, or run:

```sh
flatpak run org.ticonnect.TiConnect
```

The UI starts the local daemon automatically. Keep the app in the foreground
during tester sessions; the Flatpak path does not manage host systemd units or
autostart entries.

## Test checklist

On two LAN-connected machines:

1. Confirm both machines appear as discovered users.
2. Send text in both directions.
3. Send a file from `Downloads` in both directions.
4. Start and end a voice or video call.
5. Play a YouTube or uploaded video where supported.
6. Request remote desktop from one machine.
7. Approve the portal request on the host.
8. Verify the viewer sees the host desktop.
9. Use the kill switch or end the session.
10. Quit and relaunch TiConnect, then confirm messaging still works.

If discovery or transport fails, check:

```sh
flatpak run org.ticonnect.TiConnect network status
flatpak run org.ticonnect.TiConnect network check
```

Firewalls, VPNs, guest isolation, and Docker or VM NAT networking are the most
common causes.

## Report results

Include:

- OS, desktop, display server, and audio server.
- The exact checklist step that failed.
- `ticonnect-doctor.json`.
- Relevant daemon or UI errors.
- Whether both machines were physical devices, VMs, or containers.

Do not include private keys, fingerprints, message contents, attachments, or
screenshots containing private conversations unless explicitly requested.

## Uninstall

Remove the application and its sandbox data:

```sh
flatpak uninstall --user --delete-data org.ticonnect.TiConnect
```

## Fallback

If Flatpak is unavailable, use the release AppImage:

```sh
chmod +x ticonnect-<tag>-x86_64.AppImage
./ticonnect-<tag>-x86_64.AppImage --install-user
```

The AppImage currently validates basic installation, CLI status, and daemon
startup. Full media and remote-desktop testing are better supported through the
Flatpak bundle or a native installation.
