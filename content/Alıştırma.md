

{p -> q, q -> p, p ^ ~q}

| p q | ~q  | p -> q | q -> p | p ^ ~q |
| --- | --- | ------ | ------ | ------ |
| D D | Y   | D      | D      | Y      |
| D Y | D   | Y      | D      | D      |
| Y D | Y   | D      | Y      | Y      |
| Y Y | D   | D      | D      | Y      |
Tutarlı değil


{p V q, q V r, ~p V ~r, ~q}

| p q r | ~p ~q ~r | p V q | q V r | ~p V ~r | ~q  |
| ----- | -------- | ----- | ----- | ------- | --- |
| D D D | Y Y Y    | D     | D     | Y       | Y   |
| D D Y | Y Y D    | D     | Y     | Y       | Y   |
| D Y D | Y D Y    | D     | Y     | D       | D   |
| D Y Y | Y D D    | D     | Y     | D       | D   |
| Y D D | D Y Y    | D     | D     | D       | Y   |
| Y D Y | D Y D    | D     | Y     | D       | Y   |
| Y Y D | D D Y    | Y     | Y     | D       | D   |
| Y Y Y | D D D    | Y     | Y     | D       | D   |
Tutarlı değil 

{p -> q,  q -> r, ~(p ->r)} - ===Kontrol et===

| p q r | **p -> q** | **q -> r** | p -> r | **~(p ->r)** |
| ----- | ---------- | ---------- | ------ | ------------ |
| D D D | D          | D          | D      | Y            |
| D D Y | D          | Y          | Y      | D            |
| D Y D | Y          | D          | D      | Y            |
| D Y Y | Y          | Y          | Y      | D            |
| Y D D | D          | D          | D      | Y            |
| Y D Y | D          | D          | D      | Y            |
| Y Y D | D          | D          | D      | Y            |
| Y Y Y | D          | D          | D      | Y            |

p <-> q, q V p, |= q => Geçerli

p <->, q V p, ~q => Tutarsız

| p q | p <-> q | p v q | ~q  |
| --- | ------- | ----- | --- |
| D D | D       | D     | Y   |
| D Y | Y       | D     | D   |
| Y D | Y       | D     | Y   |
| Y Y | D       | Y     | D   |

==(p v q) -> (p ^ r), ~q |= ~r==



p -> (q ^ r), ~r V q |= p => Geçersiz

{p -> (q ^ r), ~r V q, ~p} => Tutarlı 

| p q r | ~r  | (q ^ r) | p -> (q ^ r) | ~r V q | ~p  |
| ----- | --- | ------- | ------------ | ------ | --- |
| D D D | Y   | D       | D            | D      | Y   |
| D D y | D   | Y       | Y            | D      | Y   |
| D Y D | Y   | Y       | Y            | Y      | Y   |
| D Y Y | D   | Y       | Y            | D      | Y   |
| Y D D | Y   | D       | D            | D      | D   |
| Y D Y | D   | Y       | D            | D      | D   |
| Y Y D | Y   | Y       | D            | Y      | D   |
| Y Y Y | D   | Y       | D            | D      | D   |


{p -> q, ~ p v ~q, q -> p} => Tutarlı

| p q | ~p ~q | p -> q | ~ p v ~q | q -> p |
| --- | ----- | ------ | -------- | ------ |
| D D | Y Y   | D      | Y        | D      |
| D Y | Y D   | Y      | D        | D      |
| Y D | D Y   | D      | D        | Y      |
| Y Y | D D   | D      | D        | D      |

### Saçmalama
((p v q) -> p) ^ ((p v q) -> r) === (p v q) -> (p v r)

| p q r | p v q | (p v q) -> p | (p v q) -> r | ((pvq)->p)v((pvq)->r) |
| ----- | ----- | ------------ | ------------ | --------------------- |
| D D D | D     | D            | D            | D                     |
| D D Y | D     | D            | Y            | D                     |
| D Y D | D     | D            | D            | D                     |
| D Y Y | D     | D            | Y            | D                     |
| Y D D | D     | Y            | D            | D                     |
| Y D Y | D     | Y            | Y            | Y                     |
| Y Y D | Y     | D            | D            | D                     |
| Y Y Y | Y     | D            | D            | D                     |


| p q r | p v q | p v r | (p v q) -> (p v r) |
| ----- | ----- | ----- | ------------------ |
| D D D | D     | D     | D                  |
| D D Y | D     | D     | D                  |
| D Y D | D     | D     | D                  |
| D Y Y | D     | D     | D                  |
| Y D D | D     | D     | D                  |
| Y D Y | D     | Y     | Y                  |
| Y Y D | Y     | D     | D                  |
| Y Y Y | Y     | Y     | D                  |
|       |       |       |                    |

