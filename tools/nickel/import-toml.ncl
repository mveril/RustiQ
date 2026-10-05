let Rebuild = import "rebuild-data.ncl" in
let ResolveInput = import "resolve.ncl" in
# Import the real, complete source file, without copying or translating it.
ResolveInput (Rebuild (import "../../samples/h2/sto-3g/calculation.toml"))
