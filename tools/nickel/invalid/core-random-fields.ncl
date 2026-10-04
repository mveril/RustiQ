let ResolveInput = import "../resolve.ncl" in
ResolveInput { basis.name = "sto-3g", method.hf.guess = {
  type = "CoreHamiltonian", distribution = "Uniform", min = -1, max = 1,
} }
