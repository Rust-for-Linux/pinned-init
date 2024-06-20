{
  nixConfig = {
    extra-substituters = [
      "https://nix-community.cachix.org"
    ];
    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    fenix = {
      url = "github:nix-community/fenix/staging";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    fenix,
    nixpkgs,
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      overlays = [fenix.overlays.default];
      inherit system;
    };
  in {
    packages."${system}".default = fenix.packages."${system}".minimal.toolchain;
    devShells."${system}".default = pkgs.mkShell {
      name = "rust";
      packages = with pkgs; [
        (fenix.packages."${system}".complete.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
        ])
        cargo-expand
        cargo-rdme
        cargo-semver-checks
        rust-analyzer-nightly
      ];
      shellHook = ''exec fish'';
    };
  };
}
