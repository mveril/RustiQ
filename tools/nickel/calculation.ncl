# POC frontend contract. Defaults mirror src/runfile; no scientific execution.
let IntegerBetween = fun min max => std.contract.from_predicate (fun x =>
  std.is_number x && std.number.is_integer x && x >= min && x <= max) in
let Positive = std.contract.from_predicate (fun x => std.is_number x && x > 0) in
let NonNegative = std.contract.from_predicate (fun x => std.is_number x && x >= 0) in
let NonEmpty = std.contract.from_predicate (fun x => std.is_string x && x != "") in
let Choice = fun choices => std.contract.from_predicate (fun x =>
  std.is_string x && std.array.elem x choices) in
let Nullable = fun Contract => std.contract.custom (fun label value =>
  if value == null then 'Ok null else std.contract.check Contract label value) in
let Distribution = std.contract.custom (fun label value =>
  let tag = value.distribution in
  if tag == "Normal" then std.contract.check {
    distribution | Choice ["Normal"],
    mean | Number,
    std_dev | Positive,
    seed | Nullable (IntegerBetween 0 18446744073709551615) | default = null,
  } label value
  else if tag == "Uniform" then
    let checked = std.contract.apply {
      distribution | Choice ["Uniform"],
      min | Number,
      max | Number,
      seed | Nullable (IntegerBetween 0 18446744073709551615) | default = null,
    } label value in
    if checked.max > checked.min then 'Ok checked
    else 'Error { message = "uniform distribution max must be greater than min" }
  else 'Error { message = "distribution must be Normal or Uniform" }) in
let Perturbation = std.contract.custom (fun label value =>
  if std.record.has_field "distribution" value then
    std.contract.check Distribution label value
  else std.contract.check Distribution label (value & {
    distribution | default = "Normal",
    mean | default = 0,
    std_dev | default = 0.0001,
  })) in
let Guess = std.contract.custom (fun label value =>
  let tag = std.record.get_or "type" "CoreHamiltonian" value in
  if tag == "CoreHamiltonian" || tag == "OneElectron" then
    std.contract.check {
      type | Choice ["CoreHamiltonian", "OneElectron"] | default = "CoreHamiltonian",
      perturbation | Nullable Perturbation | default = null,
    } label value
  else if tag == "Zero" then std.contract.check { type | Choice ["Zero"] } label value
  else if tag == "Random" then
    let random = std.record.remove "type" value in
    let normalized = if std.record.has_field "distribution" random then
      std.contract.apply Distribution label random
    else std.contract.apply Distribution label (random & {
      distribution | default = "Uniform", min | default = -1, max | default = 1,
    }) in
    'Ok (normalized & { type = "Random" })
  else 'Error { message = "unknown density guess type" }) in
let Hf = {
  method | Choice ["Auto", "Rhf", "Uhf"] | default = "Auto",
  max_iterations | IntegerBetween 1 9223372036854775807 | default = 100,
  convergence_threshold | Positive | default = 0.00000001,
  guess | Guess | default = {},
  diis | {
    enabled | Bool | default = false,
    max_history | IntegerBetween 2 9223372036854775807 | default = 6,
  } | default = {},
  orthogonalization | {
    linear_dependency_threshold | NonNegative | default = 0.00000001,
  } | default = {},
} in
{
  molecule | {
    geometry | NonEmpty | default = "./molecule.xyz",
    charge | IntegerBetween (-2147483648) 2147483647 | default = 0,
    multiplicity | IntegerBetween 1 255 | default = 1,
    units | Choice ["Angstrom", "Bohr"] | default = "Angstrom",
  } | default = {},
  basis | { name | String },
  method | {
    # HF is always materialized after resolution, even when omitted by input.
    hf | Hf | default = {},
    mp2 | Nullable {
      frozen_orbitals | IntegerBetween 0 9223372036854775807 | default = 0,
      # Byte-size parsing remains a Rust concern in this POC.
      memory_limit | String | default = "auto",
    } | default = null,
  } | default = {},
  integrals | { schwarz_threshold | NonNegative | default = 0.000000000001 } | default = {},
  cache | { enabled | Bool | default = false } | default = {},
  output | { scf | Choice ["Normal", "Quiet"] | default = "Normal" } | default = {},
}
