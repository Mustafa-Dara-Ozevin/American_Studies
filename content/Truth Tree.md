

---
title: "Truth Table"
tags:
aliases:
  - "[]"
---

  ---  
   
# Truth Table  
  
## Summary  
*Briefly summarize the concept here.*  
## Notes  

```tree
problem: p ∧ q, p ∴ q
1. p ∧ q  [premise] ✓
2. p  [premise]
3. ¬q  [neg-goal]
4. p  [from 1: ∧]
5. q  [from 1: ∧]
× (3, 5)
verdict: valid (all branches closed ×)
```


```tree
problem: p ∨ q, ¬p ∴ q
1. p ∨ q  [premise] ✓
2. ¬p  [premise]
3. ¬q  [neg-goal]
├─ 4. p  [from 1: ∨]
│  × (2, 4)
└─ 5. q  [from 1: ∨]
   × (3, 5)
verdict: valid (all branches closed ×)
```



```tree
problem: p → q, p ∴ q
1. p → q  [premise] ✓
2. p  [premise]
3. ¬q  [neg-goal]
├─ 4. ¬p  [from 1: →]
│  × (2, 4)
└─ 5. q  [from 1: →]
   × (3, 5)
verdict: valid (all branches closed ×)
```



```tree
problem: p → q, p ∴ ¬q
1. p → q  [premise] ✓
2. p  [premise]
3. ¬¬q  [neg-goal]
├─ 4. ¬p  [from 1: →]
│  × (2, 4)
└─ 5. q  [from 1: →]
verdict: incomplete
```



### level 2 

```tree
problem: p → q, p ∴ q
1. p → q  [premise] ✓
2. p  [premise]
3. ¬q  [neg-goal]
├─ 4. ¬p  [from 1: →]
│  × (2, 4)
└─ 5. q  [from 1: →]
   × (3, 5)
verdict: valid (all branches closed ×)
```

```tree
problem: p → q, q ∴ p
1. p → q  [premise] ✓
2. q  [premise]
3. ¬p  [neg-goal]
├─ 4. ¬p  [from 1: →]
│  o (p=Y, q=D)
└─ 5. q  [from 1: →]
   o (p=Y, q=D)
verdict: invalid (counterexample: p=Y, q=D)
```


```tree
problem: p → q, q → r ∴ p → r
1. p → q  [premise] ✓
2. q → r  [premise] ✓
3. ¬(p → r)  [neg-goal] ✓
4. p  [from 3: ¬→]
5. ¬r  [from 3: ¬→]
├─ 6. ¬p  [from 1: →]
│  × (4, 6)
└─ 7. q  [from 1: →]
   ├─ 8. ¬q  [from 2: →]
   │  × (7, 8)
   └─ 9. r  [from 2: →]
      × (5, 9)
verdict: valid (all branches closed ×)
```


### level 3 

```tree
problem: p → q, r → s, ¬q ∨ ¬s ∴ ¬p ∨ ¬r
1. p → q  [premise] ✓
2. r → s  [premise] ✓
3. ¬q ∨ ¬s  [premise] ✓
4. ¬(¬p ∨ ¬r)  [neg-goal] ✓
5. ¬¬p  [from 4: ¬∨] ✓
6. ¬¬r  [from 4: ¬∨] ✓
7. p  [from 5: ¬¬]
8. r  [from 6: ¬¬]
├─ 9. ¬p  [from 1: →]
│  × (5, 9)
└─ 10. q  [from 1: →]
   ├─ 11. ¬r  [from 2: →]
   │  × (6, 11)
   └─ 12. s  [from 2: →]
      ├─ 13. ¬q  [from 3: ∨]
      │  × (10, 13)
      └─ 14. ¬s  [from 3: ∨]
         × (12, 14)
verdict: valid (all branches closed ×)
```

```tree
problem: p → q, q → r ∴ ¬r → ¬p
1. p → q  [premise] ✓
2. q → r  [premise] ✓
3. ¬(¬r → ¬p)  [neg-goal] ✓
4. ¬r  [from 3: ¬→]
5. ¬¬p  [from 3: ¬→] ✓
6. p  [from 5: ¬¬]
├─ 7. ¬p  [from 1: →]
│  × (5, 7)
└─ 8. q  [from 1: →]
   ├─ 9. ¬q  [from 2: →]
   │  × (8, 9)
   └─ 10. r  [from 2: →]
      × (4, 10)
verdict: valid (all branches closed ×)
```






---  

### **Related:**    
- [[Map of Content related to this]]
