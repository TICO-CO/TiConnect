{
  description = "TiConnect flake package and NixOS module";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.ticonnect;
        ticonnect = pkgs.rustPlatform.buildRustPackage {
          pname = "ticonnect";
          version = "0.1.0";

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
            pkgs.gtk4
            pkgs.gtk4-layer-shell
            pkgs.libadwaita
            pkgs.sqlite
            pkgs.wayland
          ];

          doCheck = false;

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

      nixosModules.default =
        { pkgs, ... }:
        {
          imports = [ ./nix/module.nix ];
          services.ticonnect.package = self.packages.${pkgs.stdenv.hostPlatform.system}.ticonnect;
        };
    };
}
