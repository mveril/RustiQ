let Calculation = import "calculation.ncl" in
std.array.map (fun basis_name => {
  molecule.geometry = "../../samples/h2/molecule.xyz",
  basis.name = basis_name,
  method.hf = {},
  method.mp2 = {},
} | Calculation) ["sto-3g", "6-31g", "cc-pvdz"]
