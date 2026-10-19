1. It is not true that if Ron doesn't do his homework then Hermione will finish it for him
	1. r => Ron does his homework
	2. h => Hermione will finish it for him(Ron)
	3. ~(~r -> h)
	4. ~r ^ ~h
~(~r -> h) === ~r ^ ~h

| r h | ~r  | ~h  | (~r -> h) | **~(~r -> h)** | **~r ^ ~h** |
| --- | --- | --- | --------- | -------------- | ----------- |
| D D | Y   | Y   | D         | Y              | Y           |
| D Y | Y   | D   | D         | Y              | Y           |
| Y D | D   | Y   | D         | Y              | Y           |
| Y Y | D   | D   | Y         | D              | D           |



3. Harry will be singed unless he evades the dragon's fiery breath
	1. p = Harry will be singed
	2. q = harry evades the dragon's fiery breath
	3. ~q -> p

4. (p ^ ~q) v (~p ^ q)

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
5.  j, t, b |= k

| j t b k | j   | t     | b   | ~k  |
| ------- | --- | ----- | --- | --- |
| D D D D | D   | D     | D   | Y   |
| D D D Y | *D* | *D*   | *D* | *D* |
| D D Y D | D   | D     | Y   | Y   |
| D D Y Y | D   | D     | Y   | D   |
| D Y D D | D   | Y     | D   | Y   |
| D Y D Y | D   | Y     | D   | D   |
| D Y Y D | D   | Y     | Y   | Y   |
| D Y Y Y | D   | Y     | Y   | D   |
| Y D D D | Y   | D     | D   | Y   |
| Y D D Y | Y   | D     | D   | D   |
| Y D Y D | Y   | D     | Y   | Y   |
| Y D Y Y | Y   | D     | Y   | D   |
| Y Y D D | Y   | Y     | D   | Y   |
| Y Y D Y | Y   | Y<br> | D   | D   |
| Y Y Y D | Y   | Y     | Y   | Y   |
| Y Y Y Y | Y   | Y     | Y   | D   |
