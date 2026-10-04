let Calculation = import "calculation.ncl" in
{
  molecule.geometry = "../../samples/h2/molecule.xyz",
  basis.name = "sto-3g",
  method.hf = {},
  method.mp2 = {},
} | Calculation
