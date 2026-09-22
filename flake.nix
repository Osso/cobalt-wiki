{
  description = "Native Cobalt Company Wikijump packages";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/9b696460ac78b5ccfc17c854d8c976f20456e943";

  outputs =
    { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      packages.${system} = import ./install/nixos/packages.nix { inherit pkgs; };
      formatter.${system} = pkgs.nixfmt;
    };
}
