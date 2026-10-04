# Normalize one calculation or an array to the final Rust boundary directly.
let Calculation = import "calculation.ncl" in
fun value => {
  calculations =
    (if std.is_array value then value else [value]) | Array Calculation,
}
