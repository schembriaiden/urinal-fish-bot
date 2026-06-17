{
  description = "Development environment for Urinal Fish";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit overlays system;
        };
        rust = pkgs.rust-bin.stable."1.96.0".default.override {
          extensions = [
            "clippy"
            "rust-src"
            "rustfmt"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo-nextest
            cargo-watch
            pkg-config
            sqlx-cli
            sqlite
            rust
          ];

          env = {
            RUST_BACKTRACE = "1";
          };

          shellHook = ''
            echo "Urinal Fish dev shell"
            echo "Rust: $(rustc --version)"
            echo "Try: cargo test"
          '';
        };
      }
    );
}
