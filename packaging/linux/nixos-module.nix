{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.openhid;
in
{
  options.programs.openhid = {
    enable = lib.mkEnableOption "OpenHID, a local-first Logitech device manager";

    package = lib.mkPackageOption pkgs "openhid" { };

    launchAtLogin = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to start the OpenHID agent with graphical sessions.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.udev.packages = [ cfg.package ];

    systemd.user.services.openhid-agent = {
      description = "OpenHID background agent";
      wantedBy = lib.optionals cfg.launchAtLogin [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      partOf = lib.optionals cfg.launchAtLogin [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = lib.getExe' cfg.package "openhid-agent";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
