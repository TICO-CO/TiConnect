{
  description = "TiConnect flake package and NixOS module";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      gstPluginPathFor = pkgs: pkgs.lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0" [
        pkgs.gst_all_1.gstreamer
        pkgs.gst_all_1.gst-plugins-base
        pkgs.gst_all_1.gst-plugins-good
        pkgs.gst_all_1.gst-plugins-bad
        pkgs.gst_all_1.gst-plugins-ugly
        pkgs.gst_all_1.gst-libav
        pkgs.pipewire
      ];
      buildInputsFor = pkgs: [
        pkgs.alsa-lib
        pkgs.gdk-pixbuf
        pkgs.glib-networking
        pkgs.gtk4
        pkgs.gtk4-layer-shell
        pkgs.libsoup_3
        pkgs.sqlite
        pkgs.wayland
        pkgs.webkitgtk_6_0
      ];
      nativeBuildInputsFor = pkgs: [
        pkgs.cmake
        pkgs.nasm
        pkgs.pkg-config
        pkgs.perl
      ];
      runtimeInputsFor = pkgs: [
        pkgs.yt-dlp
        pkgs.gst_all_1.gst-libav
        pkgs.gst_all_1.gst-plugins-bad
        pkgs.gst_all_1.gst-plugins-base
        pkgs.gst_all_1.gst-plugins-good
        pkgs.gst_all_1.gst-plugins-ugly
        pkgs.gst_all_1.gstreamer
        pkgs.pipewire
      ];
    in
    {
      packages = forAllSystems (pkgs: let gstPluginPath = gstPluginPathFor pkgs; in {
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.ticonnect;
        ticonnect = pkgs.rustPlatform.buildRustPackage {
          pname = "ticonnect";
          version = "0.1.1";

          src = self;

          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "--bin"
            "ticonnect"
          ];

          nativeBuildInputs = nativeBuildInputsFor pkgs ++ [
            pkgs.wrapGAppsHook4
          ];

          buildInputs = buildInputsFor pkgs ++ runtimeInputsFor pkgs;

          doCheck = false;

          preFixup = ''
            gappsWrapperArgs+=(--prefix GST_PLUGIN_SYSTEM_PATH_1_0 : "${gstPluginPath}")
            gappsWrapperArgs+=(--prefix GST_PLUGIN_SYSTEM_PATH : "${gstPluginPath}")
            gappsWrapperArgs+=(--prefix GIO_EXTRA_MODULES : "${pkgs.glib-networking}/lib/gio/modules")
            gappsWrapperArgs+=(--set TICONNECT_YTDLP "${pkgs.yt-dlp}/bin/yt-dlp")
          '';

          postInstall = ''
            install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.desktop \
              "$out/share/applications/org.ticonnect.TiConnect.desktop"
            install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.svg \
              "$out/share/icons/hicolor/scalable/apps/org.ticonnect.TiConnect.svg"
            install -Dm644 packaging/flatpak/org.ticonnect.TiConnect.metainfo.xml \
              "$out/share/metainfo/org.ticonnect.TiConnect.metainfo.xml"
          '';

          meta = {
            description = "LAN-only messaging, calls, and remote desktop";
            homepage = "https://github.com/TICO-CO/TiConnect";
            license = pkgs.lib.licenses.gpl3Plus;
            mainProgram = "ticonnect";
            platforms = [ "x86_64-linux" ];
          };
        };
      });

      devShells = forAllSystems (pkgs: let gstPluginPath = gstPluginPathFor pkgs; in {
        default = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputsFor pkgs;

          buildInputs = buildInputsFor pkgs ++ runtimeInputsFor pkgs ++ [
            pkgs.alsa-plugins
            pkgs.appstream
            pkgs.bun
            pkgs.cargo
            pkgs.cargo-audit
            pkgs.clippy
            pkgs.curl
            pkgs.dbus
            pkgs.desktop-file-utils
            pkgs.git
            pkgs.grim
            pkgs.jq
            pkgs.kdePackages.kwin
            pkgs.kdePackages.xdg-desktop-portal-kde
            pkgs.libxkbcommon
            pkgs.nodejs
            pkgs.psmisc
            pkgs.python3
            pkgs.rustc
            pkgs.rustfmt
            pkgs.systemd
            pkgs.wayland-protocols
            pkgs.weston
            pkgs.wtype
            pkgs.xdotool
            pkgs.xdg-desktop-portal
            pkgs.libx11
            pkgs.libxi
            pkgs.libxtst
            pkgs.xvfb
            pkgs.zellij
          ];

          shellHook = ''
            unset CARGO_BUILD_RUSTFLAGS
            export CARGO_ENCODED_RUSTFLAGS=""
            export GST_PLUGIN_SYSTEM_PATH_1_0="${gstPluginPath}"
            export GST_PLUGIN_SYSTEM_PATH="$GST_PLUGIN_SYSTEM_PATH_1_0"
            export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
            export TICONNECT_YTDLP="${pkgs.yt-dlp}/bin/yt-dlp"
          '';
        };
      });

      nixosModules.default =
        { pkgs, ... }:
        {
          imports = [ ./nix/module.nix ];
          services.ticonnect.package = self.packages.${pkgs.stdenv.hostPlatform.system}.ticonnect;
        };
    };
}
