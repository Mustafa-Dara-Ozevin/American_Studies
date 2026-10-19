---
title: "Mantıksal kurallar"
tags:
aliases:
  - "[]"
---

  ---  
   
# Mantıksal kurallar  
  
## Summary  
Matematik gibi mantıkta da bazı işlem kuralları vardır
## Notes  

### Yer değişimi
"Ve", "veya" ve "Ancak ve ancak" da terimlerin yeri değişebilir
1. pVq === qVp
[[Eşdeğerlik]] **Kanıtı**: 
(p v q) <-> (q v p)

| p q | p ^ q | q ^ p | (p v q) <-> (q v p) |
| --- | ----- | ----- | ------------------- |
| D D | D     | D     | D                   |
| D Y | Y     | Y     | D                   |
| Y D | Y     | Y     | D                   |
| Y Y | Y     | Y     | D                   |

2. p ^ q === q ^ p
3. p <-> q === q <-> p
[[Eşdeğerlik]] **kanıtı**:

| p q | p <-> q | q <-> p | (p <-> q) <-> (q <-> p) |
| --- | ------- | ------- | ----------------------- |
| D D | D       | D       | D                       |
| D Y | Y       | Y       | D                       |
| Y D | Y       | Y       | D                       |
| Y Y | D       | D       | D                       |
### [[De Morgan Kuralı]]
Değili içeriye dağıtımızda terimlerin tersi ile birlikte ana eklemin de terisi alırız.

***~(p v q) === ~p ^ ~q****
[[Eşdeğerlik]] kanıtı:

| p q | ~p  | ~q  | p V q | ~ (p v q) | ~ p ^ ~q | ~ (p v q) <-> ~p ^ ~q |
| --- | --- | --- | ----- | --------- | -------- | --------------------- |
| D D | Y   | Y   | D     | Y         | Y        | D                     |
| D Y | Y   | D   | D     | Y         | Y        | D                     |
| Y D | D   | Y   | D     | Y         | Y        | D                     |
| Y Y | D   | D   | Y     | D         | D        | D                     |
***~(p ^ q) === ~p v ~q***


***p -> q === ~p v q***

**~(p -> q) === p ^ ~q**

[[Eşdeğerlik]] kanıtı:

| p q | ~p  | p -> q | ~ p v q | p -> q <-> ~p v q |
| --- | --- | ------ | ------- | ----------------- |
| D D | Y   | D      | D       | D                 |
| D Y | Y   | Y      | Y       | D                 |
| Y D | D   | D      | D       | D                 |
| Y Y | D   | D      | D       | D                 |

***p <-> q === (p -> q) ^ (q -> p)***


~(p -> q) === ~(~p v q)
[[De Morgan Kuralı]] => ~~ p ^ ~q
=> ~(p -> q) === p ^ ~q

[[Eşdeğerlik]] kanıtı:

| p q | ~q  | p -> q | ~(p->q) | p ^ ~q | ~(p -> q) <-> p ^ ~q |
| --- | --- | ------ | ------- | ------ | -------------------- |
| D D | Y   | D      | Y       | Y      | D                    |
| D Y | D   | Y      | D       | D      | D                    |
| Y D | Y   | D      | Y       | Y      | D                    |
| Y Y | D   | D      | Y       | Y      | D                    |

### Dağıtma Kuralı

p ^ (q v r) === ( p ^ q) V (p ^ r)

p v (q ^ r) === ( p v q) ^ (p v r)

p v (q <-> r) === (p v q) <-> (p v r)

[[Eşdeğerlik]] kanıtı:

p v (q <-> r) <-> (p v q) <-> (p v r)

| p q r | q <->r | p v (q<->r) | (p v q) | (p v r) | (p v q) <-> (p v r) |
| ----- | ------ | ----------- | ------- | ------- | ------------------- |
| D D D | D      | D           | D       | D       | D                   |
| D D Y | Y      | D           | D       | D       | D                   |
| D Y D | Y      | D           | D       | D       | D                   |
| D Y Y | D      | D           | D       | D       | D                   |
| Y D D | D      | D           | D       | D       | D                   |
| Y D Y | Y      | Y           | D       | Y       | Y                   |
| Y Y D | Y      | Y           | Y       | D       | Y                   |
| Y Y Y | D      | D           | Y       | Y       | D                   |

- [ ] p -> (q ^ r) === (p->q) ^ (p -> r)
---  

### **Related:**    
- [[Sembolik_mantık_MOC]]