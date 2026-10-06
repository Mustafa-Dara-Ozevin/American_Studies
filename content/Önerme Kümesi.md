---
title: "Önerme Kümesi"
tags:
aliases:
  - "[]"
---

  ---  
   
# Önerme Kümesi  
  
## Summary  
Tutarlı => En az bir kez hepsi doğru değerini alıyorsa
Tutarsız => Hiçbirinin hepsinde D değerini almaması
eğer önermenin tersi tutarsızsa kendisi tutarlı
## Notes  

1. {p ^ q, ~p v q, p v ~q }

| p q | p ^ q | ~p  | p v ~q | ~q  | ~p v q |
| --- | ----- | --- | ------ | --- | ------ |
| D D | D     | Y   | D      | Y   | D      |
| D Y | Y     | Y   | D      | D   | Y      |
| Y D | Y     | D   | Y      | Y   | D      |
| Y Y | Y     | D   | D      | D   | D      |
2. { p -> q, p v ~q, p}    

| p q | ~q  | p -> q | p v ~q | p   |
| --- | --- | ------ | ------ | --- |
| D D | Y   | D      | D      | D   |

3. p v q
4. p -> r
5. q -> r
6. => r



| q p r | p v q | p -> r | q -> r | ~r  |
| ----- | ----- | ------ | ------ | --- |
| D D D | D     | D      | D      | Y   |
| D D Y | D     | Y      | Y      | D   |
| D Y D | D     | D      | D      | Y   |
| D Y Y | D     | Y      | D      | D   |
| Y D D | D     | D      | D      | Y   |
| Y D Y | D     | D      | Y      | D   |
| Y Y D | Y     | D      | D      | Y   |
| Y Y Y | Y     | D      | D      | D   |
önerme tutarlı

p -> q
p
=> q

| p q | p -> q | p   | ~q  |
| --- | ------ | --- | --- |
| D D | D      | D   | Y   |
| D Y | Y      | D   | D   |
| Y D | D      | Y   | Y   |
| Y Y | D      | Y   | D   |
Tablo tutarsız çıkarım tutarlıdır


P -> q , q => ~p

| p q | p -> q | q   | p   |
| --- | ------ | --- | --- |
| D D | D      | D   | D   |
| D Y | Y      | Y   | D   |
| Y D | D      | D   | Y   |
| Y Y | D      | Y   | Y   |
Çıkarım tutarsız 

---  

### **Related:**    
- [[Map of Content related to this]]