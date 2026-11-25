+ $L = {0^n 1 0^n | n >= 0}$
  + Assume $L$ is regular
  + There exist a pumping length $P >= 1$
  + Choose $w = 0^p 1 0^p$
  + Decompose $w$ into $x y z$ such that $|x y| <= p$ and $|y| >= 1$
  + Since $|x y| <= p$, $y$ consists only of $0$s: $x = 0^alpha$, $y = 0^beta$ ($beta >= 1$), $z = 0^(p - alpha - beta) 1 0^p$.
  + Choose $i = 2$: $x y^2 z = 0^(p + beta) 1 0^p in.not L$, contradiction.
  + Therefore, $L$ is not regular. $qed$

+ $L = {0^(n^2) | n >= 0}$
  + Assume $L$ is regular
  + There exist a pumping length $p >= 1$
  + Choose $w = 0^(p^2)$
  + Decompose $w$ into $x y z$ such that $|x y| <= p$ and $|y| >= 1$
  + $x = 0^alpha$
  + $y^i = 0^(i beta)$
  + $z = 0^((p^2) - alpha - beta)$
  + Choose $i = 2$
  + $x y^2 z = 0^((p^2) + beta)$
  + ${x y^2 z} in L$ iff $(p^2) + beta$ is a perfect square
  + $beta >= 1$ then, $p^2 < p^2 + beta$
  + $beta <= P$ then, $p^2 < p^2 + beta <= p^2 + p$
  + Next perfect square is $(p+1)^2$ and $(p+1)^2 = p^2 + 2p + 1$
  + $p^2 < p^2 + beta <= p^2 + p < (p+1)^2$
  + ${x y^i z}$ is bigger than $p^2$ but smaller than the next perfect square, meaning ${x y^i z} in.not L$
  + Therefore $L$ is Non-Regular $qed$

