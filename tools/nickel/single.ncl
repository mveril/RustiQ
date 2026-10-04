let ResolveInput = import "resolve.ncl" in
ResolveInput {
  molecule.geometry = "../../samples/h2/molecule.xyz",
  basis.name = "sto-3g",
  method.hf = {},
  method.mp2 = {},
}
