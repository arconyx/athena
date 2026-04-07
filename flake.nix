{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      naersk,
      git-hooks,
      ...
    }@inputs:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      # forAllSystems :: (String -> Any) -> AttrSet
      forAllSystems = nixpkgs.lib.genAttrs systems;
      systemPkgs = system: nixpkgs.legacyPackages.${system};
    in
    {
      checks = forAllSystems (system: {
        pre-commit-check = git-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            nixfmt-rfc-style.enable = true;
            clippy.enable = true;
            rustfmt.enable = true;
          };
        };
      });

      devShells = forAllSystems (system: {
        default = (systemPkgs system).callPackage ./nix/dev.nix { };
      });

      packages = forAllSystems (system: {
        default =
          let
            naersk-lib = (systemPkgs system).callPackage naersk { };
          in
          naersk-lib.buildPackage self;
      });

      nixosModules.default = import ./nix/module.nix inputs;
    };
}
