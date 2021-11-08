{ pkgs ? import <nixpkgs> {} }:
with pkgs;
pkgs.mkShell {
  buildInputs = [ pkgconfig openssl protobuf ];
}
