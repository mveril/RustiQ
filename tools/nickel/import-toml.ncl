let ResolveInput = import "resolve.ncl" in
# Import the real, complete source file, without copying or translating it.
ResolveInput (import "../../samples/h2/sto-3g/calculation.toml")
