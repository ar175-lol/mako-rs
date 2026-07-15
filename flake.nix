{
  description = "A lightweight Wayland notification daemon written in Rust";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };
  outputs = inputs @ {
    flake-parts,
    self,
    ...
  }:
    let
      mkMakoRs = pkgs: {
        fontFamily ? "JetBrains Mono",
        fontSize ? 12.0,
        bodyFontSize ? 14.0,
        bgColor ? "(0.12, 0.12, 0.18)",
        textColor ? "(0.8, 0.84, 0.96)",
        borderColor ? "(0.54, 0.71, 0.98)",
        borderSize ? 2.0,
        padding ? 15.0,
        width ? 360,
        minHeight ? 80,
        maxBufferHeight ? 1024,
        maxVisible ? 5,
        gap ? 5,
        topMargin ? 20,
        rightMargin ? 20,
        timeoutSecs ? 5,
        sweepMs ? 100,
      }: pkgs.rustPlatform.buildRustPackage {
        pname = "mako-rs";
        version = "0.1.0";
        src = ./.;
        cargoLock = { lockFile = ./Cargo.lock; };
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ pkgs.cairo pkgs.wayland pkgs.libxkbcommon ];
        postPatch = ''
          substituteInPlace src/main.rs \
            --replace 'const FONT_FAMILY: &str = "JetBrains Mono";' 'const FONT_FAMILY: &str = "${fontFamily}";' \
            --replace 'const FONT_SIZE: f64 = 12.0;' 'const FONT_SIZE: f64 = ${toString fontSize};' \
            --replace 'const BODY_FONT_SIZE: f64 = 14.0;' 'const BODY_FONT_SIZE: f64 = ${toString bodyFontSize};' \
            --replace 'const BG_COLOR: (f64, f64, f64) = (0.12, 0.12, 0.18);' 'const BG_COLOR: (f64, f64, f64) = ${bgColor};' \
            --replace 'const TEXT_COLOR: (f64, f64, f64) = (0.8, 0.84, 0.96);' 'const TEXT_COLOR: (f64, f64, f64) = ${textColor};' \
            --replace 'const BORDER_COLOR: (f64, f64, f64) = (0.54, 0.71, 0.98);' 'const BORDER_COLOR: (f64, f64, f64) = ${borderColor};' \
            --replace 'const BORDER_SIZE: f64 = 2.0;' 'const BORDER_SIZE: f64 = ${toString borderSize};' \
            --replace 'const PADDING: f64 = 15.0;' 'const PADDING: f64 = ${toString padding};' \
            --replace 'const WIDTH: i32 = 360;' 'const WIDTH: i32 = ${toString width};' \
            --replace 'const MIN_HEIGHT: i32 = 80;' 'const MIN_HEIGHT: i32 = ${toString minHeight};' \
            --replace 'const MAX_BUFFER_HEIGHT: i32 = 1024;' 'const MAX_BUFFER_HEIGHT: i32 = ${toString maxBufferHeight};' \
            --replace 'const MAX_VISIBLE: usize = 5;' 'const MAX_VISIBLE: usize = ${toString maxVisible};' \
            --replace 'const GAP: i32 = 5;' 'const GAP: i32 = ${toString gap};' \
            --replace 'const TOP_MARGIN: i32 = 20;' 'const TOP_MARGIN: i32 = ${toString topMargin};' \
            --replace 'const RIGHT_MARGIN: i32 = 20;' 'const RIGHT_MARGIN: i32 = ${toString rightMargin};' \
            --replace 'const TIMEOUT_LOW_NORMAL: Duration = Duration::from_secs(5);' 'const TIMEOUT_LOW_NORMAL: Duration = Duration::from_secs(${toString timeoutSecs});' \
            --replace 'const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_millis(100);' 'const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_millis(${toString sweepMs});'
        '';

        meta.mainProgram = "mako-rs";
      };
    in
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" ];
      perSystem = { pkgs, ... }: {
        packages.default = mkMakoRs pkgs { };
        packages.mako-rs-custom = mkMakoRs pkgs { };
        checks.mako-rs-options = let
          testArgs = {
            fontFamily = "Fira Code";
            fontSize = 16.0;
            bodyFontSize = 13.0;
            bgColor = "(0.05, 0.05, 0.10)";
            textColor = "(0.95, 0.95, 0.99)";
            borderColor = "(0.90, 0.30, 0.30)";
            borderSize = 4.0;
            padding = 22.0;
            width = 420;
            minHeight = 100;
            maxBufferHeight = 2048;
            maxVisible = 8;
            gap = 12;
            topMargin = 40;
            rightMargin = 30;
            timeoutSecs = 9;
            sweepMs = 250;
          };
          patched = pkgs.runCommand "mako-rs-options-check" {
            src = ./.;
          } ''
            cp -r $src src
            chmod -R u+w src
            cd src
            substituteInPlace src/main.rs \
              --replace 'const FONT_FAMILY: &str = "JetBrains Mono";' 'const FONT_FAMILY: &str = "${testArgs.fontFamily}";' \
              --replace 'const FONT_SIZE: f64 = 12.0;' 'const FONT_SIZE: f64 = ${toString testArgs.fontSize};' \
              --replace 'const BODY_FONT_SIZE: f64 = 14.0;' 'const BODY_FONT_SIZE: f64 = ${toString testArgs.bodyFontSize};' \
              --replace 'const BG_COLOR: (f64, f64, f64) = (0.12, 0.12, 0.18);' 'const BG_COLOR: (f64, f64, f64) = ${testArgs.bgColor};' \
              --replace 'const TEXT_COLOR: (f64, f64, f64) = (0.8, 0.84, 0.96);' 'const TEXT_COLOR: (f64, f64, f64) = ${testArgs.textColor};' \
              --replace 'const BORDER_COLOR: (f64, f64, f64) = (0.54, 0.71, 0.98);' 'const BORDER_COLOR: (f64, f64, f64) = ${testArgs.borderColor};' \
              --replace 'const BORDER_SIZE: f64 = 2.0;' 'const BORDER_SIZE: f64 = ${toString testArgs.borderSize};' \
              --replace 'const PADDING: f64 = 15.0;' 'const PADDING: f64 = ${toString testArgs.padding};' \
              --replace 'const WIDTH: i32 = 360;' 'const WIDTH: i32 = ${toString testArgs.width};' \
              --replace 'const MIN_HEIGHT: i32 = 80;' 'const MIN_HEIGHT: i32 = ${toString testArgs.minHeight};' \
              --replace 'const MAX_BUFFER_HEIGHT: i32 = 1024;' 'const MAX_BUFFER_HEIGHT: i32 = ${toString testArgs.maxBufferHeight};' \
              --replace 'const MAX_VISIBLE: usize = 5;' 'const MAX_VISIBLE: usize = ${toString testArgs.maxVisible};' \
              --replace 'const GAP: i32 = 5;' 'const GAP: i32 = ${toString testArgs.gap};' \
              --replace 'const TOP_MARGIN: i32 = 20;' 'const TOP_MARGIN: i32 = ${toString testArgs.topMargin};' \
              --replace 'const RIGHT_MARGIN: i32 = 20;' 'const RIGHT_MARGIN: i32 = ${toString testArgs.rightMargin};' \
              --replace 'const TIMEOUT_LOW_NORMAL: Duration = Duration::from_secs(5);' 'const TIMEOUT_LOW_NORMAL: Duration = Duration::from_secs(${toString testArgs.timeoutSecs});' \
              --replace 'const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_millis(100);' 'const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_millis(${toString testArgs.sweepMs});'
            echo "--- patched src/main.rs consts ---"
            grep '^const ' src/main.rs
            fail=0
            check() {
              if ! grep -qF "$1" src/main.rs; then
                echo "MISSING: $1"
                fail=1
              fi
            }
            check 'const FONT_FAMILY: &str = "Fira Code";'
            check 'const FONT_SIZE: f64 = 16.000000;'
            check 'const BODY_FONT_SIZE: f64 = 13.000000;'
            check 'const BG_COLOR: (f64, f64, f64) = (0.05, 0.05, 0.10);'
            check 'const TEXT_COLOR: (f64, f64, f64) = (0.95, 0.95, 0.99);'
            check 'const BORDER_COLOR: (f64, f64, f64) = (0.90, 0.30, 0.30);'
            check 'const BORDER_SIZE: f64 = 4.000000;'
            check 'const PADDING: f64 = 22.000000;'
            check 'const WIDTH: i32 = 420;'
            check 'const MIN_HEIGHT: i32 = 100;'
            check 'const MAX_BUFFER_HEIGHT: i32 = 2048;'
            check 'const MAX_VISIBLE: usize = 8;'
            check 'const GAP: i32 = 12;'
            check 'const TOP_MARGIN: i32 = 40;'
            check 'const RIGHT_MARGIN: i32 = 30;'
            check 'const TIMEOUT_LOW_NORMAL: Duration = Duration::from_secs(9);'
            check 'const EXPIRY_SWEEP_INTERVAL: Duration = Duration::from_millis(250);'
            if [ "$fail" -ne 0 ]; then
              echo "One or more options failed to substitute — see MISSING lines above."
              exit 1
            fi
            echo "All 17 options substituted correctly."
            touch $out
          '';
        in patched;
      };
      flake.homeModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.mako-rs;
          toRustTuple = list: "(${lib.concatStringsSep ", " (map toString list)})";
          package = mkMakoRs pkgs {
            inherit (cfg)
              fontFamily
              fontSize
              bodyFontSize
              borderSize
              padding
              width
              minHeight
              maxBufferHeight
              maxVisible
              gap
              topMargin
              rightMargin
              timeoutSecs
              sweepMs
              ;
            bgColor = toRustTuple cfg.bgColor;
            textColor = toRustTuple cfg.textColor;
            borderColor = toRustTuple cfg.borderColor;
          };
        in {
          options.programs.mako-rs = {
            enable = lib.mkEnableOption "mako-rs notification daemon";
            fontFamily = lib.mkOption { type = lib.types.str; default = "JetBrains Mono"; };
            fontSize = lib.mkOption { type = lib.types.float; default = 12.0; };
            bodyFontSize = lib.mkOption { type = lib.types.float; default = 14.0; };
            borderSize = lib.mkOption { type = lib.types.float; default = 2.0; };
            padding = lib.mkOption { type = lib.types.float; default = 15.0; };
            width = lib.mkOption { type = lib.types.int; default = 360; };
            minHeight = lib.mkOption { type = lib.types.int; default = 80; };
            maxBufferHeight = lib.mkOption { type = lib.types.int; default = 1024; };
            maxVisible = lib.mkOption { type = lib.types.int; default = 5; };
            gap = lib.mkOption { type = lib.types.int; default = 5; };
            topMargin = lib.mkOption { type = lib.types.int; default = 20; };
            rightMargin = lib.mkOption { type = lib.types.int; default = 20; };
            bgColor = lib.mkOption { type = lib.types.listOf lib.types.float; default = [ 0.12 0.12 0.18 ]; };
            textColor = lib.mkOption { type = lib.types.listOf lib.types.float; default = [ 0.8 0.84 0.96 ]; };
            borderColor = lib.mkOption { type = lib.types.listOf lib.types.float; default = [ 0.54 0.71 0.98 ]; };
            timeoutSecs = lib.mkOption { type = lib.types.int; default = 5; };
            sweepMs = lib.mkOption { type = lib.types.int; default = 100; };
          };
          config = lib.mkIf cfg.enable {
            home.packages = [ package ];

            systemd.user.services.mako-rs = {
              Unit = {
                Description = "mako-rs notification daemon";
                Documentation = "https://github.com/ar175-lol/mako-rs";
                PartOf = [ "graphical-session.target" ];
                After = [ "graphical-session.target" ];
              };
              Service = {
                ExecStart = lib.getExe package;
                Restart = "on-failure";
                RestartSec = 2;
              };
              Install = {
                WantedBy = [ "graphical-session.target" ];
              };
            };
          };
        };
    };
}
