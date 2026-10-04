let ResolveInput = import "../resolve.ncl" in
ResolveInput { basis.name = "sto-3g", method.hf.guess = {
  type = "Random", distribution = "Uniform", min = -1, max = 1,
  mean = 0, std_dev = 1,
} }
