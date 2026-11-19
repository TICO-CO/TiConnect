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
    in
    {
      packages = forAllSystems (pkgs: let gstPluginPath = gstPluginPathFor pkgs; in {
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.ticonnect;
        ticonnect = pkgs.rustPlatform.buildRustPackage {
          pname = "ticonnect";
          version = "0.0.6";

          src = self;

          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [
            "--bin"
            "ticonnect"
          ];

          nativeBuildInputs = [
            pkgs.cmake
            pkgs.pkg-config
            pkgs.wrapGAppsHook4
          ];

          buildInputs = [
            pkgs.alsa-lib
            pkgs.gdk-pixbuf
            pkgs.glib-networking
            pkgs.yt-dlp
            pkgs.gst_all_1.gst-libav
            pkgs.gst_all_1.gst-plugins-bad
            pkgs.gst_all_1.gst-plugins-base
            pkgs.gst_all_1.gst-plugins-good
            pkgs.gst_all_1.gst-plugins-ugly
            pkgs.gst_all_1.gstreamer
            pkgs.gtk4
            pkgs.gtk4-layer-shell
            pkgs.pipewire
            pkgs.sqlite
            pkgs.wayland
            pkgs.webkitgtk_6_0
          ];

          doCheck = false;

          preFixup = ''
            gappsWrapperArgs+=(--prefix GST_PLUGIN_SYSTEM_PATH_1_0 : "${gstPluginPath}")
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
          nativeBuildInputs = [
            pkgs.cmake
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.alsa-lib
            pkgs.gdk-pixbuf
            pkgs.glib-networking
            pkgs.yt-dlp
            pkgs.gst_all_1.gst-libav
            pkgs.gst_all_1.gst-plugins-bad
            pkgs.gst_all_1.gst-plugins-base
            pkgs.gst_all_1.gst-plugins-good
            pkgs.gst_all_1.gst-plugins-ugly
            pkgs.gst_all_1.gstreamer
            pkgs.gtk4
            pkgs.gtk4-layer-shell
            pkgs.bun
            pkgs.nodejs
            pkgs.pipewire
            pkgs.sqlite
            pkgs.wayland
            pkgs.webkitgtk_6_0
          ];

          shellHook = ''
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
