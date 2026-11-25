// Shared vault helpers for clickable Obsidian wikilinks inside Typst PDFs.
// Usage:
//   #import "../_lib/vault.typ": wiki
//   See #wiki("Menon Diyaloğu") or #wiki("Pumping Lemma Exercise", display: "Pumping Lemma").

#let _hex = "0123456789ABCDEF"

#let _url-encode(s) = {
  let out = ""
  for b in bytes(s) {
    let is-unreserved = (
      (b >= 65 and b <= 90) or
      (b >= 97 and b <= 122) or
      (b >= 48 and b <= 57) or
      b == 45 or b == 46 or b == 95 or b == 126 or b == 47
    )
    if is-unreserved {
      out += str(bytes((b,)))
    } else {
      let hi = calc.quo(b, 16)
      let lo = calc.rem(b, 16)
      out += "%" + _hex.at(hi) + _hex.at(lo)
    }
  }
  out
}

#let wiki(target, display: none, vault: "my-notes") = {
  let label = if display != none { display } else { target }
  let uri = "obsidian://open?vault=" + _url-encode(vault) + "&file=" + _url-encode(target)
  link(uri, text(fill: rgb("#5c7cfa"), label))
}
