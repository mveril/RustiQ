let Calculation = import "../calculation.ncl" in
{ basis.name = "sto-3g", method.hf.guess.type = "Bogus" } | Calculation
