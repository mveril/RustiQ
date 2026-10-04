let Calculation = import "../calculation.ncl" in
{ basis.name = "sto-3g", method.hf.diis.max_history = 1 } | Calculation
