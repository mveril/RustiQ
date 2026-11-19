{
  frozen_orbitals | std.contract.from_predicate (fun x =>
    std.is_number x && std.number.is_integer x && x >= 0 && x <= 9223372036854775807)
    | default = 0,
  # Byte-size parsing remains a shared Rust domain responsibility.
  memory_limit | String | default = "auto",
}
