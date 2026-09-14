let
  pkgs = import (builtins.fetchTarball {
    url = "https://releases.nixos.org/nixpkgs/nixpkgs-26.11pre1070774.d5dfd8e6716d/nixexprs.tar.xz";
    sha256 = "sha256-pAelOjF4w4iqEJozlFGQmWMobMa6F9eLIbMeBPBW7Ts=";
  }) {};
in pkgs.mkShell {
  packages = [ pkgs.nodejs pkgs.chromium ];
}
