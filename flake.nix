{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
  };

  outputs =
    {
      nixpkgs,
      ...
    }:
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
      devShells = forAllSystems (system: {
        default = (systemPkgs system).callPackage ./nix/dev.nix { };
      });

      packages = forAllSystems (system: {
        default = (systemPkgs system).callPackage ./nix/package.nix { };
      });

      nixosModules.default = import ./nix/module.nix;
    };
}
