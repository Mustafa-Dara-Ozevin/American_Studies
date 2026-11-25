/// Render a semantic tableau (çözümleyici çizelge) from structured entries.
#let cizelge(
  bracket-stroke: 0.7pt,
  bracket-width: 0.45em,
  closed-color: red,
  open-color: green.darken(20%),
  column-gutter: (0.6em, 0.8em, 0.5em),
  row-gutter: 0.45em,
  group-spacing: 0.15em,
  breakable: false,
  ..entries,
) = block(breakable: breakable, {
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
      cells.push(grid.cell(colspan: 4, v(group-spacing)))
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
            stroke: (
              top: bracket-stroke,
              right: bracket-stroke,
              bottom: bracket-stroke,
            ),
            inset: 0pt,
          )[])
          cells.push(grid.cell(rowspan: n, align: left + horizon, entry.ref))
        }
      }
    } else if entry.kind == "closed" {
      cells.push([])
      cells.push(text(fill: closed-color, weight: "bold", $times$))
      cells.push([])
      cells.push([])
    } else if entry.kind == "open" {
      cells.push([])
      cells.push(text(fill: open-color, weight: "bold", $circle.small$))
      cells.push([])
      cells.push([])
    }
  }

  grid(
    columns: (auto, auto, bracket-width, auto),
    column-gutter: column-gutter,
    row-gutter: row-gutter,
    align: (right + horizon, center + horizon, center, left + horizon),
    ..cells,
  )
})
