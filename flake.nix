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
          # rust = pkgs.rust-bin.stable.latest.default.override {
          #   extensions = [
          #     "rust-src"
          #     "rust-analyzer"
          #   ];
          # };
          rust = pkgs.rust-bin.selectLatestNightlyWith (
            toolchain:
            toolchain.default.override {
              extensions = [
                "rust-src"
                "rust-analyzer"
              ];
            }
          );
          libPath =
            with pkgs;
            lib.makeLibraryPath [
            ];
        in
        {
          default = pkgs.mkShell (
            {
              nativeBuildInputs = [
                rust
                pkgs.cargo-insta
                pkgs.python3 # run.py test harness
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.mold-unwrapped ];
              LD_LIBRARY_PATH = libPath;
            }
            # Reference toolchain for run.sh: *unwrapped* gcc, so the NixOS
            # cc-wrapper can't inject hardening/search flags — run.sh owns
            # every setting the reference compiler runs with. The unwrapped
            # compiler knows nothing about libc, so REFCC_FLAGS hands it
            # glibc's headers, crt objects, and dynamic linker explicitly.
            // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
              REFCC = "${pkgs.gcc-unwrapped}/bin/gcc";
              REFCC_FLAGS = builtins.concatStringsSep " " [
                "-B${pkgs.glibc}/lib"
                "-L${pkgs.glibc}/lib"
                "-I${pkgs.glibc.dev}/include"
                "-L${pkgs.gcc-unwrapped.lib}/lib" # libgcc_s.so
                # -fuse-ld=mold resolves ld.mold via -B, not PATH — so the
                # *unwrapped* mold links, not nixpkgs' bintools-wrapper (which
                # injects relro/bindnow hardening and auto-rpaths).
                "-B${pkgs.mold-unwrapped}/bin"
                "-Wl,-dynamic-linker,${pkgs.glibc}/lib/ld-linux-x86-64.so.2"
                "-Wl,-rpath,${pkgs.glibc}/lib"
                "-Wl,-rpath,${pkgs.gcc-unwrapped.lib}/lib"
              ];
            }
          );
        }
      );
    };
}
