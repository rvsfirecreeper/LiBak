{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      # winit/Slint dlopen these at runtime, so linking against them isn't enough:
      # they have to be findable at runtime too (LD_LIBRARY_PATH in the dev shell,
      # RUNPATH in the packaged binary).
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
      packages.x86_64-linux.default = pkgs.rustPlatform.buildRustPackage {
        pname = "libak";
        version = "0.1.0";
        src = self;
        cargoLock.lockFile = ./Cargo.lock;

        nativeBuildInputs = [ pkgs.pkg-config pkgs.patchelf ];
        # fontconfig/freetype/zstd are linked; the rest are only dlopen'ed (see runtimeLibs).
        buildInputs = runtimeLibs ++ [ pkgs.zstd ];

        postFixup = ''
          patchelf --add-rpath "${pkgs.lib.makeLibraryPath runtimeLibs}:/run/opengl-driver/lib" \
            $out/bin/libak
        '';

        meta = {
          description = "Encrypted, deduplicating backup tool";
          license = pkgs.lib.licenses.gpl3Plus;
          mainProgram = "libak";
        };
      };

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
