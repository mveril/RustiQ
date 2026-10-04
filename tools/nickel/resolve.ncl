# Normalize one calculation or an array to the final Rust boundary directly.
let Calculation = import "calculation.ncl" in
let NonEmptyCalculations = std.contract.custom (fun label value =>
  let calculations = if std.is_array value then value else [value] in
  if std.array.length calculations == 0 then
    'Error { message = "at least one calculation is required" }
  else
    std.contract.check (Array Calculation) label calculations) in
fun value => {
  calculations = value | NonEmptyCalculations,
}
