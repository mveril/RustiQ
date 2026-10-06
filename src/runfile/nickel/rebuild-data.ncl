# Work around Nickel 2.2 debug assertions when merging deserialized containers.
# Rebuild every level so imported data can safely receive record contracts.
let rec Rebuild = fun value =>
  if std.is_record value then
    std.record.map (fun _ field => Rebuild field) value
  else if std.is_array value then
    std.array.map Rebuild value
  else value
in Rebuild
