{
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "athena";
  # This should be safe from IFD because we are pointing to a file in
  # this repository instead of a derivation
  version = (builtins.fromTOML (builtins.readFile ./../Cargo.toml)).package.version;

  src = ./..;

  cargoLock.lockFile = ./../Cargo.lock;
}
