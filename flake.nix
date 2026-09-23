{
  description = "Native Cobalt Company Wikijump packages";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/9b696460ac78b5ccfc17c854d8c976f20456e943";
  # Caches Rust dependencies in their own derivation, so deploys rebuild only Deepwell.
  inputs.crane.url = "github:ipetkov/crane";

  outputs =
    { nixpkgs, crane, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      packages.${system} = import ./install/nixos/packages.nix {
        inherit pkgs;
        craneLib = crane.mkLib pkgs;
      };
      formatter.${system} = pkgs.nixfmt;
    };
}
