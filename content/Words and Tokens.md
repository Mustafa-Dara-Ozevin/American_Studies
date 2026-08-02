---
title: Words and Tokens
tags:
  - ai
  - digital_humanities
  - nlp
aliases:
  - "[]"
---

  ---  
   
# Words and Tokens  
  
## Summary  
*Briefly summarize the concept here.*  
## Notes  
- **[[Tokenization]]**, the act of out or tokenizing words from running text, is the first step of the modern [[NLP]]
- How to reperesent words in characters? UTF-8, unicode, morpheme?
- **[[BPE(Byte-pair encoding)]]** is an algorithm that automatically breaks up input text into tokens 
- **[[Edit Distance]]** is an important metric that measures how similar to words are to each other based on the number of edits it takes to turn one into another

## Words

> They picnicked by the pool, then lay back on the grass and
looked at the stars.

- The sentence above has 16 or 18 words depending on if we count the punctuation or not. Punctuation is generally counted as separate words in LLMs
- Spoken language introduces even more complications. What about *disfluencies* should they count as separate words?

>I do uh main- mainly business data processing

- This utterance has 2 different disfluencies: "main-" is a fragment and "uh" is a filler.
- Disfluencies could be stripped away or kept depending on the mission
- **[[Word type]]**: are the number of the ==distinct words==
- **[[Word instance]]**: is the ==total number of running words==
- **[[Orthographic Words]]** => Latin writing system but can't be applied to every language easily 
- There are 2 classes of words: **Function words** like of, a, and, etc. and **Content Words** that tends to have meanings about places, people and events.
- **Word Types** are infinite as **content words** grows indefinitely as we encounter more and more running text.
- We will constantly encounter *unknown words*.
- Because of many languages not having Orthographic Words and constant unknown words, NLP models use **subwords** or **[[Morphemes]]** as the processing unit

## Morphemes: Parts of Words

- A morpheme is the minimal meaning-bearing unit in a language.
- The word cats consists of two: the morpheme cat and the morpheme -s that indicates plural.
- **[[Morphemes]]** has 2 categories: *roots* and *affixes*
- **Affixes** broadly has 2 categories *inflectional morphemes* and *derivational morphemes*
- There is another class of morphemes: **clitics**. A **clitic** is a morpheme that acts syntactically like a word but is reduced in form and attached to another word.
- The study of how languages vary in their morphology, i.e., how words break up into their parts, is called **[[morphological typology]]**.
- **Number of morphemes per word** some languages have just over 1 [[Morphemes]] per word. These languages are called [[Isolating Languages]]. a single word may have very many [[Morphemes]] e call languages toward this end of the scale [[Synthetic Languages]].  and the very end of the scale [[Polysynthetic Languages]]
- ![[Pasted image 20260717163918.png]]
- **Segment-ability of the [[Morphemes]]**: ranging from [[Agglutinative languages]] like Turkish, in which morphemes have relatively clean boundaries to  [[Fusion languages]] like Russian, in which a single affix may conflate multiple morphemes.
- the fact morphemes can be hard to define, and that many languages can have complex morphemes that aren’t easy to break up into pieces makes it ==very difficult to use morphemes as a standard for tokenization cross-lingually==.
- 



---  

### **Related:**    
- [[Map of Content related to this]]