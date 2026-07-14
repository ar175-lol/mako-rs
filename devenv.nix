{pkgs, ...}: {
  languages.rust = {
    enable = true;
    channel = "stable";
    #toolchain = [
    #  "rustc"
    #  "cargo"
    #  "rustfmt"
    #  "rust-analyzer"
    #  "rust-src"
    #];
  };

  packages = [
    pkgs.pkg-config
    pkgs.libxkbcommon
    pkgs.cairo
  ];
}
