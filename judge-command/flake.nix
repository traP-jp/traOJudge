{
  description = "traOJudge judge command flakes.";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-latest.url = "github:NixOS/nixpkgs/nixos-25.11";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    inputs@{
      flake-parts,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;

      imports = [
        ./lib
        ./languages
      ];

      perSystem =
        { config, ... }:
        {
          packages.default = config.trao.languages.cpp-23-gcc.internal.prebuild;
        };
    };
}
