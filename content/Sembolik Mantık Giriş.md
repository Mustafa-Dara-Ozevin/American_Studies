---
title: "Sembolik Mantık Giriş"
tags:
aliases:
  - "[]"
---

  ---  
   
# Sembolik Mantık Giriş  
  
## Summary  
Tümdengelimsel ve Tümevarımsal ifadeler. Herhangi bir simge sembol olarak kullanılabilir
## Notes  

- Tümdengelim: A, B, C / q, p, r
- Tümevarım: A1, A2, A3, A4, A5, A6 ... An => B
- ~ = değil
- ^ = ve
- v = ya da 
- -> = ise
- <-> = ancak ve ancak 
- Ali yürüdü ve başını çarptı
- p ^ q
- Ali çalışırsa geç kalmaz
- p -> q
- Türkiyenin başkenti Ankara'dır ve 2 + 2 dört'tür
- p ^ q
- p ve q ana bileşen, eklem(^) de bir bileşendir

### Değil

| p   | ~p  |
| --- | --- |
| D   | Y   |
| Y   | D   |
### Ve

| p q | p ^ q |
| --- | ----- |
| D D | D     |
| D Y | Y     |
| Y D | Y     |
| Y Y | Y     |

### Ya da

| p q | p v q |
| --- | ----- |
| D D | D     |
| D Y | D     |
| Y D | D     |
| Y Y | Y     |

### ise

| p  q | p -> q |
| ---- | ------ |
| D D  | D      |
| D Y  | Y      |
| Y D  | D      |
| Y Y  | D      |

### ancak ve ancak

| p   q | p <-> q |
| ----- | ------- |
| D   D | D       |
| D  Y  | Y       |
| Y  D  | Y       |
| Y  Y  | D       |

P v q
~q
=> p 

| p  q | p v q | ~q  | p   |
| ---- | ----- | --- | --- |
| D  D | D*    | Y*  | D*  |
| D  Y | D     | D   | D   |
| Y  D | D     | Y   | Y   |
| Y  Y | Y     | D   | Y   |

- p -> q === ~p v q

| p q | ~p  | ~p v q |
| --- | --- | ------ |
| D D | Y   | D      |
| D Y | Y   | Y      |
| Y D | D   | D      |
| Y D | D   | D      |


- p <-> q === (p -> q) ^ (q -> p)




***Modus Ponens***: Her önerme doğru olduğunda sonucu sağlıyorsa doğrudur
p ve q


***Modus Tollens***: Eğer önermeler 


[[Totoloji]]: Bir şeyin kendine özdeş olduğunu belirtir.
Tp = T


---  

### **Related:**    
- [[Disjunctive Syllogism]]