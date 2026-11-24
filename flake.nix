{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      # winit/Slint dlopen these at runtime, so linking against them isn't enough:
      # they have to be on LD_LIBRARY_PATH too.
      runtimeLibs = with pkgs; [
        wayland
        libxkbcommon
        libGL
        vulkan-loader
        fontconfig
        freetype
        libx11
        libxcursor
        libxrandr
        libxi
      ];
    in {
      devShells.x86_64-linux.default = pkgs.mkShell {
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = runtimeLibs ++ [ pkgs.ripgrep ];
        shellHook = ''
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          # NixOS keeps the GPU driver here; harmless if it doesn't exist.
          export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:/run/opengl-driver/lib"
        '';
      };
    };
}
