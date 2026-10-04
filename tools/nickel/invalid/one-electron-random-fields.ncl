let ResolveInput = import "../resolve.ncl" in
ResolveInput { basis.name = "sto-3g", method.hf.guess = {
  type = "OneElectron", distribution = "Normal", mean = 0, std_dev = 1,
} }
