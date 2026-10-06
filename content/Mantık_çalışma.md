1. It is not true that if Ron doesn't do his homework then Hermoine will finish it for him
	1. p = Ron Does his homework
	2. q = Hermoine will finish it for him
	3. ~(~p -> q)
2. Harry will be singed unless he evades the dragon's fiery breath
	1. p = Harry will be singed
	2. q = harry evades the dragon's fiery breath
	3. ~q -> p
3. (p ^ ~q) v (~p ^ q)

| p q | ~p ~q | (p ^ ~q) | (~p ^ q) | (p ^ ~q) v (~p ^ q) |
| --- | ----- | -------- | -------- | ------------------- |
| D D | Y Y   | Y        | Y        | Y                   |
| D Y | Y D   | D        | Y        | D                   |
| Y D | D Y   | Y        | D        | D                   |
| Y Y | D D   | Y        | Y        | Y                   |
4. Aristotle was neither a great philosopher nor a great scientist
	1. p Aristotle was a great philosopher
	2. s Aristotle was a great scientist
	3. => ~p ^ ~s
5.  j, t, B |= k

| j t b k | j t K | ~k  |
| ------- | ----- | --- |
| D D D D |       |     |
| D D D Y |       |     |
| D D Y D |       |     |
| D D Y Y |       |     |
| D Y D D |       |     |
| D Y D Y |       |     |
| D Y Y D |       |     |
| D Y Y Y |       |     |
| Y D D D |       |     |
| Y D D Y |       |     |
| Y D Y D |       |     |
| Y D Y Y |       |     |
| Y Y D D |       |     |
| Y Y D Y |       |     |
| Y Y Y D |       |     |
| Y Y Y Y |       |     |