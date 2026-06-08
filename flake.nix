{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" ];
          };
          # rust = pkgs.rust-bin.selectLatestNightlyWith (
          #   toolchain:
          #   toolchain.default.override {
          #     extensions = [ "rust-src" ];
          #   }
          # );
          libPath =
            with pkgs;
            lib.makeLibraryPath [
            ];
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = [ rust ];
            LD_LIBRARY_PATH = libPath;
          };
        }
      );
    };
}
