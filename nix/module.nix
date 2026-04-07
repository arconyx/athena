{
  config,
  lib,
  pkgs,
  self,
  ...
}:

let
  cfg = config.services.athena;
  pkg = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
in
{
  options.services.athena = {
    enable = lib.mkEnableOption "Athena";
    databaseUrl = lib.mkOption {
      type = lib.types.str;
      default = "postgresql:///athena?host=/run/postgresql";
      description = ''
        Postgres connection string.

        See https://docs.rs/tokio-postgres/latest/tokio_postgres/config/struct.Config.html
        for details on the format
      '';
    };
    discordToken = lib.mkOption {
      type = lib.types.str;
      example = "XAASFA-FDAFAF";
      # TODO: Find a more secure alternative,
      # probably using a key file  and systemd credentials
      description = "Discord bot token";
    };
  };

  config = lib.mkIf cfg.enable {
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

      after = [
        "network-online.target"
        "postgresql.target"
      ];
      requires = [ "postgresql.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      startLimitIntervalSec = 60;
      startLimitBurst = 2;

      serviceConfig = {
        Type = "exec";
        Restart = "on-failure";
        ExecStart = "${pkg}/bin/athena";
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
      };

      environment = {
        DISCORD_TOKEN = cfg.discordToken;
        DATABASE_URL = cfg.databaseUrl;
      };
    };

  };
}
