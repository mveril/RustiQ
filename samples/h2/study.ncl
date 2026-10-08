let bases = ["sto-3g", "6-31g"] in
std.array.map
  (fun basis_name => {
    molecule.geometry = "molecule.xyz",
    basis.name = basis_name,
    method.hf.diis = { enabled = true, max_history = 2 },
  })
  bases
