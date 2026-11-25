# cizelge

Modular Typst package for semantic tableaux (*çözümleyici çizelge*) with numbered premises, bracketed decomposition groups, and closed/open branch markers.

## Usage (relative import)

```typst
#import "cizelge/lib.typ": cizelge, bo, satir, ac, kapali, acik

#cizelge(
  bo(1, $not p and (q and not r)$),
  bo(2, $not q and r$),
  ac([(1)],
    $not p$,
    (3, $q and not r$),
  ),
  ac([(2)],
    $not q$,
    $r$,
  ),
  ac([(3)],
    $q$,
    $not r$,
  ),
  kapali,
)
```

## Installing as a `@local` Typst package

To import from anywhere as `#import "@local/cizelge:0.1.0": *`:

```bash
mkdir -p ~/.local/share/typst/packages/local/cizelge
ln -s /home/mustapha/Stuff/Math/logic/cizelge ~/.local/share/typst/packages/local/cizelge/0.1.0
```
