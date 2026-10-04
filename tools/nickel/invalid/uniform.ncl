let Calculation = import "../calculation.ncl" in
{ basis.name = "sto-3g", method.hf.guess = {
  type = "Random", distribution = "Uniform", min = 1, max = -1,
} } | Calculation
