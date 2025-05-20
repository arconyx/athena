# Athena
A Discord bot written in Rust. Intended to have very low memory use. This is written for personal use and not as a generic bot framework.

## Installation
This repository includes a NixOS module that can be used to run the bot as a systemd service. If you don't want to use NixOS
grab the latest binary from GitHub Actions (or build it yourself). Athena
expects a postgres database to be available at runtime.

It reads the `DISCORD_TOKEN` and `DATABASE_URL` environment variables at launch. `.env` files are not presently supported.
The Discord token is a bot token from the Discord developer portal. No privileged intents are required.
The database url format is specified in the [tokio-postgres::Config](https://docs.rs/tokio-postgres/0.7.13/tokio_postgres/config/struct.Config.html) object.

## Development
The bare minimum needed to build the project is Rust and Cargo. If you wish to run it locally you'll also want a PostgreSQL instance.

### Setup with Nix
This project includes a [nix](https://nixos.org/) flake with a dev shell defined, so nix users can simply `cd` into the project root and run
```sh
nix develop
```
This will download some stuff and drop you into a shell with rust, cargo, postgres and a few other useful tools (like clippy).


This requires having the nix experimental featues `nix-command` and `flakes` enabled. If you don't, the following command should work
```sh
nix --extra-experimental-features "nix-command flakes" develop
```
But if you're going to be doing that more than once you'll probably just
want to enable those features in your [nix config](https://nix.dev/manual/nix/2.24/command-ref/conf-file).

### Setup without Nix
Install Rust and Cargo however you like. Remember to install PostgreSQL too, if you want to be able to run the bot locally. At time of writing the Nix flake uses v1.82 of rust and cargo. Postgres is at v16.8. Other recent version should also work fine.

### Building
```sh
cargo build
```
That's all.

If you want to do a release build for some reason use `cargo build --release` but be warned it is much slower as the build process is optimised to minimise the final binary size.

### Running
`cargo run` will compile and run it locally. Remember to set the `DISCORD_TOKEN` and `DATABASE_URL` environment variables, as described above.

If you want to spin up a temporary postgres instance try the helper script in the repository root. This will create a database stored in the `$PGDATA` dir and connect to it over a unix socket stored in the `$PGHOST` dir. The nix dev shell sets those enviroment variables to point into subfolders of `/tmp/pg`, creating it if it does not exist.

If you're using a bash shell (you probably are) then source the file to setup some helper aliases
```sh
source ./start-postgres.sh
```

Else just run it normally
```sh
./start-postgres.sh
```
If you're not using the nix flake remember to set the `PGDATA` and `PGHOST` environment variables appropriately first.
