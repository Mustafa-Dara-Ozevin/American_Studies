#let bo(no, form) = (kind: "single", no: no, form: form, ref: [(B.Ö.)])
#let satir(no, form, ref: []) = (kind: "single", no: no, form: form, ref: ref)
#let ac(ref, ..items) = (kind: "group", ref: ref, items: items.pos())
#let kapali = (kind: "closed")

#let cizelge(..entries) = block(breakable: false, {
  let cells = ()
  for entry in entries.pos() {
    if entry.kind == "single" {
      let num = if entry.no != none { [#entry.no.] } else { [] }
      cells.push(num)
      cells.push(entry.form)
      cells.push([])
      cells.push(entry.ref)
    } else if entry.kind == "group" {
      let items = entry.items
      let n = items.len()
      cells.push(grid.cell(colspan: 4, v(0.15em)))
      for (idx, item) in items.enumerate() {
        let (num, form) = if type(item) == array {
          ([#item.at(0).], item.at(1))
        } else {
          ([], item)
        }
        cells.push(num)
        cells.push(form)
        if idx == 0 {
          cells.push(grid.cell(
            rowspan: n,
            stroke: (top: 0.7pt, right: 0.7pt, bottom: 0.7pt),
            inset: 0pt,
          )[])
          cells.push(grid.cell(rowspan: n, align: left + horizon, entry.ref))
        }
      }
    } else if entry.kind == "closed" {
      cells.push([])
      cells.push(text(fill: red, weight: "bold", $times$))
      cells.push([])
      cells.push([])
    }
  }
  grid(
    columns: (auto, auto, 0.45em, auto),
    column-gutter: (0.6em, 0.8em, 0.5em),
    row-gutter: 0.45em,
    align: (right + horizon, center + horizon, center, left + horizon),
    ..cells,
  )
})

= Çizelge

+ $A_1 and A_2 and A_3 ... and A_n$
  + $A_1$
  + $A_2$
  + ...
  + $A_n$

= Examples

+ $not p and not q$
  #cizelge(
    bo(1, $not p and not q$),
    ac([(1)], $not p$, $not q$),
  )

+ $(not p and not q) and (r and not q)$
  #cizelge(
    bo(1, $(not p and not q) and (r and not q)$),
    ac([(1)], (2, $not p and not q$), (3, $r and not q$)),
    ac([(2)], $not p$, $not q$),
    ac([(3)], $r$, $not q$),
  )

+ $(p and not r) and (q and not r) and (p and q)$
  #cizelge(
    bo(1, $(p and not r) and (q and not r) and (p and q)$),
    ac([(1)], (2, $p and not r$), (3, $q and not r$), (4, $p and q$)),
    ac([(2)], $p$, $not r$),
    ac([(3)], $q$, $not r$),
    ac([(4)], $p$, $q$),
  )

= Proposition sets

+ ${not p and (q and not r), not q and r, not p and r}$
  #cizelge(
    bo(1, $not p and (q and not r)$),
    bo(2, $not q and r$),
    bo(3, $not p and r$),
    ac([(1)], $not p$, (4, $q and not r$)),
    ac([(2)], $not q$, $r$),
    ac([(3)], $not p$, $r$),
    ac([(4)], $q$, $not r$),
    kapali,
  )

+ ${p and q, p and r}$
  #cizelge(
    bo(1, $p and q$),
    bo(2, $p and r$),
    ac([(1)], $p$, $q$),
    ac([(2)], $p$, $r$),
  )

+ ${p and q, p and r, not r}$
  #cizelge(
    bo(1, $p and q$),
    bo(2, $p and r$),
    bo(none, $not r$),
    ac([(1)], $p$, $q$),
    ac([(2)], $p$, $r$),
    kapali,
  )

+ ${(p and q) and r, q and r, p}$
  #cizelge(
    bo(1, $(p and q) and r$),
    bo(2, $q and r$),
    bo(none, $p$),
    ac([(1)], (3, $p and q$), $r$),
    ac([(2)], $q$, $r$),
    ac([(3)], $p$, $q$),
  )

+ ${not p and not q, p and q}$

  #cizelge(
    bo(1, $not p and not q$),
    bo(2, $p and q$),
    ac([(1)], $not p$, $not q$),
    ac([(2)], $p$, $q$),
    kapali,
  )

+ ${ not p and (q and not r), not q and r, not p and r}$

  #cizelge(
    bo(1, $not p and (q and not r)$),
    bo(2, $not q and r$),
    bo(3, $not p and r$),
    ac([(1)], $not p$, (4, $q and not r$)),
    ac([(2)], $not q and r$),
    ac([(3)], $not p and r$),
    ac([(4)], $q and not r$),
    kapali,
  )

+ ${(p and q) and not r, (not p and not r) and (not p and q)}$

  #cizelge(
    bo(1, ${(p and q) and not r}$),
    bo(2, $(not p and not r) and (not p and q)$),
    ac([(1)], (3, $p and q$), $not r$),
    ac([(2)], (4, $not p and not r$), (5, $not p and q$)),
    ac([(3)], $p$, $q$),
    ac([(4)], $not p$, $not r$),
    ac([(5)], $not p$, $q$),
    kapali,
  )

+ ${not p, not p and q, not p and q and r}$

  #cizelge(
    bo(1, $not p$),
    bo(2, $not p and q$),
    bo(3, $not p and q and r$),
    ac([(1)], $not p$, $q$),
    ac([(2)], $not p$, $q$, $r$),
  )

= Ödev

+ $p, q : 2 "önerme"$ biri yanlış biri doğru sonra bununla $p and q, p or q, not q$ nasıl eklemlenir bir örnek yaz, doğruluk tablosu çıkar.
