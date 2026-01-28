{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      darwinDeps = pkgs: pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
        pkgs.apple-sdk_15
        (pkgs.darwinMinVersionHook "10.14")
      ];
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
          };
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [
              rustToolchain
              pkgs.rdkafka
              pkgs.openssl
              pkgs.cyrus_sasl
              pkgs.zstd
            ] ++ darwinDeps pkgs;
          };
        });

      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "kafka_oneshot";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [
              pkgs.rdkafka
              pkgs.openssl
              pkgs.cyrus_sasl
              pkgs.zstd
            ] ++ darwinDeps pkgs;
          };
        });
    };
}
