---
title: Eşdeğerlik
tags:
aliases:
  - "[]"
---

  ---  
   
# Eşdeğerlik  
  
## Summary  
Eşdeğerlik: === 
## Notes  

p -> q === ~p v q 
=> (p -> q) <-> ~p v q

| p q | ~p  | p -> q | ~p v q | (p -> q) <-> ~p v q |
| --- | --- | ------ | ------ | ------------------- |
| D D | Y   | D      | D      | D                   |
| D Y | Y   | Y      | Y      | D                   |
| Y D | D   | D      | D      | D                   |
| Y Y | D   | D      | D      | D                   |
|     |     |        |        |                     |

P ^ Q ===  p ^ (p -> q)

| p q | p ^ q | p -> q | p ^ (p -> q) | P ^ Q <->  p ^ (p -> q) |
| --- | ----- | ------ | ------------ | ----------------------- |
| D D | D     | D      | D            | D                       |
| D Y | Y     | Y      | Y            | D                       |
| Y D | Y     | D      | Y            | D                       |
| Y Y | Y     | D      | Y            | D                       |
    
---  

### **Related:**    
- [[Map of Content related to this]]