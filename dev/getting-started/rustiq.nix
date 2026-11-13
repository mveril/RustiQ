# NixOS module: provide the RustiQ flake through specialArgs.inputs.
{ pkgs, inputs, ... }:
{
  environment.systemPackages = [
    inputs.rustiq.packages.${pkgs.stdenv.hostPlatform.system}.default
  ];
}
