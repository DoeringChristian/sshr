{
  description = "sshr - Resilient SSH sessions with automatic reconnection";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
    in {
      packages.default = pkgs.stdenvNoCC.mkDerivation {
        pname = "sshr";
        version = "0.1.0";
        src = ./.;

        installPhase = ''
          mkdir -p $out/bin $out/share/sshr/{kitty,shpool}

          cp bin/sshr $out/bin/sshr
          chmod +x $out/bin/sshr

          cp kitty/*.py $out/share/sshr/kitty/
          cp shpool/build.sh $out/share/sshr/shpool/

          # Copy pre-built shpool binaries if present
          if [ -d shpool/bin ]; then
            cp -r shpool/bin $out/share/sshr/shpool/
          fi
        '';

        meta = with pkgs.lib; {
          description = "Resilient SSH sessions with automatic reconnection and persistent shells";
          license = licenses.mit;
          platforms = platforms.unix;
          mainProgram = "sshr";
        };
      };
    });
}
