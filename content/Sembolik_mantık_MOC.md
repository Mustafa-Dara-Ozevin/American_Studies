---
title: Sembolik_mantık_MOC
tags:
aliases:
  - Symbolic Logic
---
___

1. [[Sembolik Mantık Giriş]]
2. [[Mantık_çalışma]]
3. [[Önerme]]
4. [[Önerme Kümesi]]
5. [[Eşdeğerlik]]
6. [[içerme]]
7. [[Mantıksal kurallar]]
8. [[Modus Tollens]]
9. [[Modus Ponens]]

Kitaplar:
- [ ] Graham Priest - Mantık


Yapılacaklar:

- [x] (p <-> q) === (p ^ q) v (~p ^ ~q)

(p <-> q) <-> (p ^ q) v (~p ^ ~q)

| p q | **p <-> q** | (p ^ q) | (~p ^ ~q) | **(p ^ q) v (~p ^ ~q)** | (p <-> q) <-> (p ^ q) v (~p ^ ~q) |
| --- | ----------- | ------- | --------- | ----------------------- | --------------------------------- |
| D D | D           | D       | Y         | D                       | D                                 |
| D Y | Y           | Y       | Y         | Y                       | D                                 |
| Y D | Y           | Y       | Y         | Y                       | D                                 |
| Y Y | D           | Y       | D         | D                       | D                                 |
- [x] ( p <-> q) |= (r v p) <-> (r v q)

( p <-> q) -> (r v p) <-> (r v q)

| p q r | ***p <-> q*** | r v p | r v q | ***(r v p) <-> (r v q)*** | ( p <-> q) -> (r v p) <-> (r v q) |
| ----- | ------------- | ----- | ----- | ------------------------- | --------------------------------- |
| D D D | D             | D     | D     | D                         | D                                 |
| D D Y | D             | D     | D     | D                         | D                                 |
| D Y D | Y             | D     | D     | D                         | D                                 |
| D Y Y | Y             | D     | Y     | Y                         | D                                 |
| Y D D | Y             | D     | D     | D                         | D                                 |
| Y D Y | Y             | Y     | D     | Y                         | D                                 |
| Y Y D | D             | D     | D     | D                         | D                                 |
| Y Y Y | D             | Y     | Y     | D                         | D                                 |


- [ ] ( p <-> q) |= (r v p) <-> (r v q)
( p <-> q) -> (r v p) <-> (r v q)


| p q r | **p <-> q** | r v p | r v q | **(r v p) <-> (r v q)** |     |
| ----- | ----------- | ----- | ----- | ----------------------- | --- |
| D D D | D           | D     | D     | D                       | D   |
| D D Y | D           | D     | D     | D                       | D   |
| D Y D | Y           | D     | D     | D                       | D   |
| D Y Y | Y           | D     | Y     | Y                       | Y   |
| Y D D | Y           | D     | D     | D                       | D   |
| Y D Y | Y           | Y     | D     | Y                       | D   |
| Y Y D | D           | D     | D     | D                       | D   |
| Y Y Y | D           | Y     | Y     | D                       | D   |
