{
  mkShell,

  # vscode's integrated terminal can get weird without this
  bashInteractive,

  # rust
  cargo,
  rustc,

  # rust dev tools
  clippy,
  rustfmt,
  rust-analyzer,
  # let us point to the rust source
  rustPlatform,

  # let us test with the database
  postgresql,
}:
let
  pg_path = "/tmp/pg";
in
mkShell {
  buildInputs = [
    bashInteractive
    cargo
    rustc
    clippy
    rustfmt
    rust-analyzer
    postgresql
  ];
  RUST_SRC_PATH = rustPlatform.rustLibSrc;
  DATABASE_URL = "postgresql:///athena?user=postgres&host=${pg_path}/sockets";
  PGDATA = "${pg_path}/data";
  PGHOST = "${pg_path}/sockets";
  shellHook = "echo To start a dev database use './start-postgres.sh'";
}
