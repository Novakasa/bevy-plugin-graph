{
  description = "bevy_plugin_graph — records which Bevy plugin added which plugin";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          inherit (pkgs) lib;

          # The crate itself only needs bevy_app and bevy_ecs, which link nothing
          # unusual. These are here for downstream apps and examples that pull in
          # windowing, audio and Vulkan — Bevy dlopen()s most of them at runtime,
          # so they have to be on LD_LIBRARY_PATH rather than just linked.
          bevyRuntimeLibs = lib.optionals pkgs.stdenv.hostPlatform.isLinux (
            with pkgs;
            [
              alsa-lib
              udev
              vulkan-loader
              libxkbcommon
              wayland
              libx11
              libxcursor
              libxi
              libxrandr
            ]
          );
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rustc
              cargo
              clippy
              rustfmt
              rust-analyzer
              pkg-config
            ];

            buildInputs = bevyRuntimeLibs;

            env = {
              LD_LIBRARY_PATH = lib.makeLibraryPath bevyRuntimeLibs;
              RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
            };
          };
        }
      );
    };
}
