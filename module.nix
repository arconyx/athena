inputs:
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.services.athena;
  athena-pkg = inputs.self.packages.${pkgs.stdenv.hostPlatform.system}.default;
in
{
  options.services.athena = {
    enable = lib.mkEnableOption "Athena";
    databaseUrl = lib.mkOption {
      type = lib.types.str;
      default = "postgresql:///athena,host=/var/run/postgresql";
    };
    discordToken = lib.mkOption {
      type = lib.types.str;
      example = "XAASFA-FDAFAF";
      description = "Discord bot token"; # TODO: Find a more secure alternative, probably using a key file
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [
      athena-pkg
    ];

    services.postgresql = {
      enable = true;
      ensureDatabases = [ "athena" ];
      ensureUsers = [
        {
          name = "athena";
          ensureDBOwnership = true;
        }
      ];
    };

    systemd.services.athena = {
      description = "Athena Discord Bot";

      after = [ "network-online.target" ];
      requires = [ "postgresql.service" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      startLimitIntervalSec = 60;
      startLimitBurst = 2;

      restartTriggers = [ athena-pkg ];

      serviceConfig = {
        Type = "exec";
        Restart = "on-failure";
        ExecStart = "${athena-pkg}/bin/athena";
        RestartSec = 5;
        TimeoutStopSec = 900;

        User = "athena";
        Group = "athena";
        DynamicUser = true;

        NoNewPrivileges = true;
        RestrictNamespaces = "cgroup ipc pid user cgroup net";
        KeyringMode = "private";
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectProc = "invisible";
        ProtectClock = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
        PrivateDevices = true;
        SystemCallArchitectures = "native";
        MemoryDenyWriteExecute = true;
        ProtectHostname = true;
        LockPersonality = true;
        RestrictRealtime = true;
        UMask = "027";
      };

      environment = {
        DISCORD_TOKEN = cfg.discordToken;
        DATABASE_URL = cfg.databaseUrl;
      };
    };

  };
}
