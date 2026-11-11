# NixOS module: provide the RustiQ flake through specialArgs.inputs.
{ pkgs, inputs, ... }:
{
  environment.systemPackages = [
    (inputs.rustiq.legacyPackages.${pkgs.stdenv.hostPlatform.system}.cpuTarget "native")
  ];
}
