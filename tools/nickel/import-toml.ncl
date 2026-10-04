let Calculation = import "calculation.ncl" in
# Import the real, complete source file, without copying or translating it.
(import "../../samples/h2/sto-3g/calculation.toml") | Calculation
