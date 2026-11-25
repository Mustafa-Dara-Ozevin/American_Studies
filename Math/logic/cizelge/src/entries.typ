/// Başlangıç önermesi (initial premise) entry.
#let bo(no, form, label: [(B.Ö.)]) = (
  kind: "single",
  no: no,
  form: form,
  ref: label,
)

/// Standalone single line with optional number and right-side reference.
#let satir(no, form, ref: []) = (
  kind: "single",
  no: no,
  form: form,
  ref: ref,
)

/// Bracketed decomposition group: groups resulting lines under a `]` bracket with `ref`.
/// Each item can be either a formula (`$not p$`) or a `(number, formula)` pair (`(4, $q and not r$)`).
#let ac(ref, ..items) = (
  kind: "group",
  ref: ref,
  items: items.pos(),
)

/// Closed branch marker (`×`).
#let kapali = (kind: "closed")

/// Open branch marker (`○`).
#let acik = (kind: "open")
