---
title: "içerme"
tags:
aliases:
  - "[]"
---

  ---  
   
# içerme  
  
## Summary  
İçerme: |=
ilk önermenin ikincisinin yeterli ve gerekli koşulu olduğunu gösterir
## Notes  
p ^ q |= q ^ p
(p ^ q) -> (q ^ p)

| p q | p ^ q | q ^ p | (p ^ q) -> (q ^ p) |
| --- | ----- | ----- | ------------------ |
| D D | D     | D     | D                  |
| D Y | Y     | Y     | D                  |
| Y D | Y     | Y     | D                  |
| Y Y | Y     | Y     | D                  |
**(p ^ q) (q ^ p)'un yeterli ve gerekli koşuludur.**

p v q, p -> r, q -> r |= r
{pVq, p->r, q->r, ~r}

| p q | pVq | p->r | q->r | ~r  |
| --- | --- | ---- | ---- | --- |
|     |     |      |      |     |
|     |     |      |      |     |
|     |     |      |      |     |

| p q r | p -> q | q -> r | p -> r | ~( p -> r) |
| ----- | ------ | ------ | ------ | ---------- |
| D D D | D      | D      | D      | Y          |
| D D Y | D      | Y      | Y      | D          |
| D Y D | Y      | D      | D      | Y          |
| D Y Y | Y      | D      | Y      | D          |
| Y D D | D      | D      | D      | Y          |
| Y D Y | D      | Y      | D      | Y          |
| Y Y D | D      | D      | D      | Y          |
| Y Y Y | D      | D      | D      | Y          |

---  

### **Related:**    
- [[Map of Content related to this]]