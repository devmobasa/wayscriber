{
  description = "Wayscriber - Screen annotation tool for Wayland";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        version = "0.9.8";
      in {
        packages = {
          wayscriber = pkgs.rustPlatform.buildRustPackage {
            pname = "wayscriber";
            inherit version;
            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [ pkg-config ];

            buildInputs = with pkgs; [
              cairo
              pango
              wayland
              libxkbcommon
            ];

            postInstall = ''
              install -Dm644 packaging/wayscriber.desktop $out/share/applications/wayscriber.desktop
              install -Dm644 packaging/wayscriber.service $out/lib/systemd/user/wayscriber.service
              for size in 24 64 128; do
                install -Dm644 packaging/icons/wayscriber-$size.png \
                  $out/share/icons/hicolor/''${size}x''${size}/apps/wayscriber.png
              done
              install -Dm644 config.example.toml $out/share/doc/wayscriber/config.example.toml
              install -Dm644 README.md $out/share/doc/wayscriber/README.md
              install -Dm644 LICENSE $out/share/licenses/wayscriber/LICENSE
            '';

            meta = with pkgs.lib; {
              description = "Screen annotation tool for Wayland compositors";
              homepage = "https://wayscriber.com";
              license = licenses.mit;
              platforms = platforms.linux;
              mainProgram = "wayscriber";
            };
          };

          wayscriber-configurator = pkgs.rustPlatform.buildRustPackage {
            pname = "wayscriber-configurator";
            inherit version;
            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;
            buildAndTestSubdir = "configurator";

            nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];

            # Iced GUI toolkit dependencies
            buildInputs = with pkgs; [
              cairo
              pango
              wayland
              libxkbcommon
              vulkan-loader
              libGL
            ];

            postInstall = ''
              install -Dm644 packaging/wayscriber-configurator.desktop \
                $out/share/applications/wayscriber-configurator.desktop
              for size in 24 64 128; do
                install -Dm644 packaging/icons/wayscriber-configurator-$size.png \
                  $out/share/icons/hicolor/''${size}x''${size}/apps/wayscriber-configurator.png
              done
              install -Dm644 README.md $out/share/doc/wayscriber-configurator/README.md
              install -Dm644 LICENSE $out/share/licenses/wayscriber-configurator/LICENSE

              # Wrap binary to find GL/Vulkan libraries at runtime (Iced uses dlopen)
              wrapProgram $out/bin/wayscriber-configurator \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
                  pkgs.vulkan-loader
                  pkgs.libGL
                  pkgs.wayland
                  pkgs.libxkbcommon
                ]}
            '';

            meta = with pkgs.lib; {
              description = "GUI configurator for wayscriber";
              homepage = "https://wayscriber.com";
              license = licenses.mit;
              platforms = platforms.linux;
              mainProgram = "wayscriber-configurator";
            };
          };

          default = self.packages.${system}.wayscriber;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc
            rust-analyzer
            clippy
            rustfmt
            pkg-config
            cairo
            pango
            wayland
            libxkbcommon
            vulkan-loader
            libGL
          ];
        };
      });
}
