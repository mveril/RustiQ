let ResolveInput = import "resolve.ncl" in
ResolveInput (std.array.map (fun guess => {
  basis.name = "sto-3g",
  method.hf.guess = guess,
}) [
  { type = "CoreHamiltonian", perturbation = {} },
  { type = "OneElectron", perturbation = { seed = 42 } },
  { type = "Random" },
  { type = "Random", distribution = "Normal", mean = 0, std_dev = 0.1, seed = 42 },
  { type = "Zero" },
])
