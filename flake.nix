{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      utils,
      naersk,
      ...
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage ./.;
        devShell =
          with pkgs;
          mkShell {
            buildInputs = [
              cargo
              rustc
              rustfmt
              pre-commit
              rustPackages.clippy
              pkg-config
              openssl
              postgresql
            ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            DATABASE_URL = "postgresql:///athena?user=postgres&host=/tmp/pg";

            shellHook = ''
              echo "Using ${pkgs.postgresql.name}"

              # Set the custom environment variables
              export PGDATA="/tmp/pg"
              export PGHOST="/tmp/pg"

              # Custom Postgres config to use Unix socket
              cat > $PGDATA/postgresql.conf <<EOF
              # Add Custom Settings
              log_directory = 'pg_log'
              log_filename = 'postgresql-%Y-%m-%d_%H%M%S.log'
              logging_collector = on

              # Unix socket settings
              unix_socket_directories = '/tmp'
              EOF

              # Post Shell Hook: Initialize DB and start Postgres
              if [ ! -d "$PGDATA" ]; then
                pg_ctl initdb -o "-U postgres"
                cat "$PGDATA/postgresql.conf" >> "$PGDATA/postgresql.conf"
              fi

              pg_ctl -o "-k $PGDATA" start
              # Create the 'athena' database if it doesn't exist
              psql -U postgres -c "SELECT 1 FROM pg_database WHERE datname = 'athena'" | grep -q 1 || psql -U postgres -c "CREATE DATABASE athena"

              alias fin="pg_ctl stop && exit"
              alias pg="psql -h /tmp -U postgres"
            '';
          };
      }
    );
}
