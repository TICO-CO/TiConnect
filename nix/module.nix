{
  config,
  lib,
  ...
}:
let
  cfg = config.services.ticonnect;
in
{
  options.services.ticonnect = {
    enable = lib.mkEnableOption "TiConnect daemon and overlay";

    package = lib.mkOption {
      type = lib.types.package;
      description = "TiConnect package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    systemd.user.services.ticonnectd = {
      description = "TiConnect background daemon";
      wantedBy = [ "default.target" ];
      after = [ "graphical-session.target" ];
      serviceConfig = {
        Type = "dbus";
        BusName = "org.ticonnect.Daemon";
        ExecStart = "${cfg.package}/bin/ticonnect daemon";
        Restart = "on-failure";
        RestartSec = 2;
      };
    };

    systemd.user.services.ticonnect-overlay = {
      description = "TiConnect corner overlay";
      wantedBy = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [
        "graphical-session.target"
        "ticonnectd.service"
      ];
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/ticonnect overlay";
        Restart = "on-failure";
        RestartSec = 3;
      };
    };
  };
}
