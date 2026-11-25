

1. L = {0^n10^n | n >= 0}
	1. Assume L === Regular
	2. There exist a P that is P > 0 
	3. w = 0^p10^p
	4. Decompose w into xyz 
	5. |xy| <= p ^ 0^p -> y can be only 0^β
	6. x = 0^α
	7. y = 0^β, β >=1 
	8. z = 0^(p-α-β)10^p
	9. xy^iz = 0^α0^iβ0^(p-α-β)10^p
	10. xy^iz = 0^(iβ+p-β)10^p
	11. <=> iβ+p - β = p
	12. <=> iβ -β = 0
	13. <=> i = 1
	14. Choose i = 2
	15. 2β = β
	16. ∴ L is non Regular 


2. L = {0^(n^2) | n >= 0}
	1. Assume L is regular 
	2. There exists pumping length P given by the Lemma, let P >= 1
	3. Choose w = 0^p^2 
	4. Decompose into xyz by Lemma 
	5. x = 0^α
	6. y = 0^β, β >= 1
	7. z = 0^p^2-0^α+β
	8. xy^2z = 0^(p^2)+ iβ - β
	9. Choose i = 2
	10. xy^iz = 0^(p^2) + β
	11. <=> p^2 + β is a perfect square
	12. β >= 1 -> p^2 < p^2 + β
	13. p >= β -> p^2 < p^2 + β <= p^2 ^ p
	14. Next perfect square is (p + 1)^2
	15. (p + 1)^2 > p^2 + p because (p + 1)^2 = p^2 + 2p + 1 and p >= 1
	16. β -> p^2 < p^2 + β <= p^2 ^ p < (p + 1)^2
	17. xy^iz is bigger than p(n^2) and smaller than the next perfect square
	18. ∴ L is non Regular 

3. L = {0^(2^n) | n >= 0}
	1. Assume L is regular 
	2. There exists pumping length P given by the Lemma, let P >= 1
	3. Choose w = 0^(2^p)
	4. x = 0^α
	5. y = 0^β, β >= 1
	6. z = 0^(2^p)-α - β
	7. Choose i = 2
	8. xy^2z = 0^(2^p) + β
	9. <=> 2^p + β is a power of 2 
	10. β >= 1 -> 2^p < 2^p + β
	11. p >= β -> 2^p < 2^p + β <= 2^p + p
	12. next power of 2 is 2^(p+1)
	13. 2^(p+1) = 2(2^p) and p >= 1
	14. 2^p > p
		1. p >= 0 
		2. Base Case: 
			1. 2^0 > 0 
			2. Hypothesis: 2^k > k for some k>= 0
			3. **Step:** 2^(k+1) = 2^k + 2^k > k + 1, because 2^k > k (hypothesis) and 2^k ≥ 1 (true for all k ≥ 0).
	15. -> p >= β -> 2^p < 2^p + β <= 2^p + p < 2^(p+1)
	16. xy^iz is bigger than (2^p) and smaller than the next power of 2
	17. Hence x y² z ∉ L, contradicting the lemma. ∴ L is not regular. 