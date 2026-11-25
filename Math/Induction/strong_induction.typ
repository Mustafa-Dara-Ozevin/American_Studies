= Strong Induction

+ Goal: ${P(n) | n >= a}$
+ Step 1(Basis): Prove ${P(a), P(a+1) ... P(n+i)}$
+ Step 2(Induction):
  + Assume $P(i) forall a <= k <= i$
  + Prove $P(k + 1)$


= Example

+ $alpha_1 = 1, alpha_2=3, alpha_(n) = alpha_(n-2) + 2alpha_(n-1)$
  + $alpha_3 = 1 + 2.3, alpha_3 = 7$
  + Claim: $alpha_k "is odd" forall k >=1$
  + Basis: {alpha_1=1, alpha_2=3} are odd
  + Induction: Assume that $alpha_i "is odd" forall 1 <= i <= k$
  + Then, $alpha_(k+1) = alpha_(n-1) + 2alpha(n)$
  + $= 2p+1 + 2(q+1) forall p,q in natural$

