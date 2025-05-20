{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      naersk,
      ...
    }@inputs:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        packages.default = naersk-lib.buildPackage ./.;
        devShells.default =
          let
            pg_path = "/tmp/pg";
          in
          pkgs.mkShell {
            buildInputs = with pkgs; [
              bashInteractive
              cargo
              rustc
              rustfmt
              pre-commit
              rustPackages.clippy
              pkg-config
              openssl
              postgresql
            ];
            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
            DATABASE_URL = "postgresql:///athena?user=postgres&host=${pg_path}/sockets";
            PGDATA = "${pg_path}/data";
            PGHOST = "${pg_path}/sockets";
            shellHook = "echo To start a dev database use './start-postgres.sh'";
          };
      }
    )
    // {
      nixosModules.default = import ./nix/module.nix inputs;
    };
}
