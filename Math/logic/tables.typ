
#import "@preview/truthfy:0.6.0": truth-table, truth-table-empty
#import "@preview/syntree:0.2.1": syntree
#import "@preview/curryst:0.6.0": prooftree, rule
#import "@preview/frederic:0.1.0": assume, premise, proof, step, subproof

= Truth Table
#truth-table(
  $(p => q) <=> (not p => not q)$,
  $(p and q) => r$,
)

= Synth tree
#syntree("[$<=>$ [$=>$ $p$ $q$] [$=>$ [$not$ $q$]  [$not$ $p$]]]")



#syntree(
  "[$p->q,r->s,¬q∨¬s,¬(¬p∨¬r)$ [$p,r$ [$¬q$ [$¬p$ $×$] [$q$ $×$]] [$¬s$ [$¬r$ $×$] [$s$ $×$]]]]",
)
