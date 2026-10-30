---
title: "Mantık Devreleri"
tags:
aliases:
  - "[]"
---

  ---  
   
# Mantık Devreleri  
  
## Summary  
[[Claude Shannon]] yüksek lisans tezinde Mantıkta kullanılan D/Y ifadesinin 1/0 olarak devrelerede fiziksel olarak ifade edilebileceğimizi kanıtlamışıtır.
## Notes  

### Tüm semboller ^ ve ~ ile ifade edilebilir
- A V B === ~(~A ^ ~B)
- A -> B === ~( A ^ ~B)

| A B | ~B  | A ^ ~B | ~( A ^ ~B) | A -> B | <-> |
| --- | --- | ------ | ---------- | ------ | --- |
| D D | Y   | Y      | D          | D      | D   |
| D Y | D   | D      | Y          | Y      | D   |
| Y D | Y   | Y      | D          | D      | D   |
| Y Y | D   | Y      | D          | D      | D   |

- A <-> B === ~(A ^ ~B) ^ ~(B ^ ~A)

| A B | ~A ~B | A ^ ~B | B ^ ~A | ~(A ^ ~B) | ~(B ^ ~A) | A <-> B | ~(A ^ ~B) ^ ~(B ^ ~A) |
| --- | ----- | ------ | ------ | --------- | --------- | ------- | --------------------- |
| D D | Y Y   | Y      | Y      | D         | D         | D       | D                     |
| D Y | Y D   | D      | Y      | Y         | D         | Y       | Y                     |
| Y D | D Y   | Y      | D      | D         | Y         | Y       | Y                     |
| Y Y | D D   | Y      | Y      | D         | D         | D       | D                     |

### Tüm denklemler V ve ~'ye indirgenebilir

- A ^ B === ~ (~A V ~ B)
- A -> B === ~A V B

| A B | ~A  | ~A V B | A -> B | A -> B <-> ~A V B |
| --- | --- | ------ | ------ | ----------------- |
| D D | Y   | D      | D      | D                 |
| D Y | Y   | Y      | Y      | D                 |
| Y D | D   | D      | D      | D                 |
| Y Y | D   | D      | D      | D                 |
- A <-> B === ~(~A v ~B) V ~(A v B)

### Tüm Denklemler -> ve ~'ye indirgenebilir

- A ^ B === ~(A->~B)
- A V B === ~A -> B
- A <-> B === [(A -> B) -> ~(B -> A)]

| A B | A -> B | B -> A | ~(B -> A) | [(A -> B) -> ~(B -> A)] | ~[(A -> B) -> ~(B -> A)] |
| --- | ------ | ------ | --------- | ----------------------- | ------------------------ |
| D D | D      | D      | Y         | Y                       | D                        |
| D Y | Y      | D      | Y         | D                       | Y                        |
| Y D | D      | Y      | D         | D                       | Y                        |
| Y Y | D      | D      | Y         | Y                       | D                        |


### Yeni semboller 

- A ↑ B === ~(A ^ B)
- A ↓ B === ~(A v B)
- A <- B === B -> A
- A ↛ B === B ~(A -> B)
- A ↚ B === ~(B->A)

↑ ↓ ⊢ ← → 

***NAND***

| p q | p ^ q | ~(p ^ q) |
| --- | ----- | -------- |
| D D | D     | Y        |
| D Y | Y     | D        |
| Y D | Y     | D        |
| Y Y | Y     | D        |

***NOR***

| p q | p v q | ~ (p v q) |
| --- | ----- | --------- |
| D D | D     | Y         |
| D Y | D     | Y         |
| Y D | D     | Y         |
| Y Y | Y     | D         |

| A   | ~A  | A ^ A | ~ (A v A) |
| --- | --- | ----- | --------- |
| D   | Y   | D     | Y         |
| Y   | D   | Y     | D         |

### Önemli İsimler 
- [[Frege]]
- [[Whitehead]]
- [[Russel]]
- [[Quine]]
- [[Pierce]]
    
---  

### **Related:**    
- [[Map of Content related to this]]