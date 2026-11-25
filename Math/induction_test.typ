#import "_lib/vault.typ": wiki

= induction test
#linebreak()

+ $1 + 2 + 3... + n = n(n +1) / 2 divides n >= 1$
  + Base Case: (n = 1) #linebreak()
    $1 = 1(1+1) / 1$
  + Inductive Hypothesis: #linebreak()
    Assume True: $1 + 2 + 3.. + k = k(k+1)/2$
  + Inductive Step:
    + $n = k + 1$
    + $limits( underbrace(1 + 2 + 3...+ k))_(k(k+1)/2) + (k+1) = ((k+1)(k+2))/2$
    + $(k(k+1)/2 + (k+1)) dot 2 = (((k+1)(k+2))/2) dot 2$
    + $k(k+1) + 2(k+1) = (k+1)(k+2)$
    + $k^2 + 3k + 2 = k^2 + 3k + 2$

+ $7^n - 1 | n >= 1$ Can always be divided to 6
  + Base Case: (n = 1) #linebreak()
    $f(1) = 7^1 -1, f(1) = 6$
  + Inductive Hyopthesis: #linebreak()
    $f(k) = 7^k -1 $
  + Inductive Step:
    + $n = k + 1$
    + $f(k + 1) -f(k) = 7^(k+1) - 1 - limits(underbrace(f(k))_(7^k - 1))$
    + $f(k+1) = 7(7^k) - 7^k + cancel(1-1) $
    + $f(k+1) = 6(7^k)$


= Links 
#wiki("MATH_MOC")
