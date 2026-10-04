let Calculation = import "../calculation.ncl" in
{ basis.name = "sto-3g", method.hf.convergence_threshold = -1 } | Calculation
