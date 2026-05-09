---
title: "Presidential action-speech Knowledge map"
tags:
---

6
Neo4J graph database
## Nodes

### President(Node)

- name
- birth_date
- OLID
- presidency_start
- presidency_end
- Profession_background
- Party
- state representation

### Person(Node)

- name
- birth_date
- OLID
- party_affiliation
- state
- profession_background

### Election (Node)

- candidate_list
- Winner
- electoral_vote_percantage
- congress_majority_party
- Datetime  

### Historical event (Node)

- Name
- Datetime
- Location
- IsDomestic
- summary

### Veto (Node)

Label: work

- Datetime
- Veto Type (Pocket, Regular)
- Sentiment score
- Full text

### Act/Bill (Node)

  

- Name
- Datetime
- Summary
- Sentiment Score
- full text

### Executive Order (Node)

 label: work

- Name
- Datetime
- Sentiment score
- Full text
- Number
- IsNumbered

### Inaugural speech (Node)

label:work

- Datetime
- Full Text
- sentiment score  

### SOTU (Node)

label:work

- Datetime
- Full Text
- sentiment score

### topic (Node)


- Name (Social equity, Foreign policy, military, economy, education etc)
- Description

### Subtopic (Node)

- Name
- Description

### Moral Value (Node)

- Name(MFT: Care, Fairness, Loyalty, Authority, Purity, Equality, Proportionality, Liberty, Honor, Ownership)
- Description
- keywords

### CulturalConcept (Node)

- Name (American Exceptionalism, Manifest Destiny, Self-reliance, Frontier, Pragmatism)
- origin_age
- definition


---

## Relationships

  
### Authorship & Direct Action

(President)-->(Work) [issues]

(President)-->(President) [Succesor_of, vice_president_of]

(Person)-->(Work) [advises_for, advises_against, advises_in] (Specifically for instances like Thomas Jefferson advising George Washington on the 1792 veto message)

(Person)-->(Act/Bill) [sponspors]

(President)-->(Election) [wins_in, loses_in]


### Legislative & Policy Friction

  
(Veto)-->(Act/Bill) [rejects_constitutionality_of] (Essential for early vetoes where the President acted as a constitutional "police" power)

(Veto)-->(Act/Bill) [rejects_policy_of] (Used when a President disagrees with the legislative direction rather than its legal basis)

(Veto)-->(Act/Bill) [rejects_applicability_of]

(Act/Bill)-->(Act/Bill) [repassed_as] (Links a vetoed bill to its later amended version that successfully passed, such as the 1792 Apportionment Act)


### Rhetorical Framing (Content Strategy)

  
(Work)-->(Subtopic) [addresses] (Generic engagement or mention of an issue)

(Work)-->(Subtopic) [advocates_for] (Positive framing of a policy or goal)

(Work)-->(Subtopic) [urges_action_to] (Specifically for State of the Union requests where the President pressures Congress)

(Subtopic)-->(Topic) [subtopic_of] (The hierarchical connection for data aggregation)

(Work)-->(MoralValue) [appeals_to] (Links rhetoric to psychological systems like Fairness or Authority)

(Work)-->(CulturalConcept) [invokes] (Connects a text to a specific ideological trope like American Exceptionalism)

  

### Historical & Contextual Mapping

  
(Work)-->(HistoricalEvent) [references] (A general mention of a past or current event)

(Work)-->(HistoricalEvent) [reacts_to] (A specific response to a crisis, such as Washington's reaction to the Whiskey Rebellion)

(Work)-->(HistoricalEvent) [celebrates] (Used for celebratory or anniversary addresses)

(Work)-->(HistoricalEvent) [warns_against]

(CulturalConcept)-->(Topic) [dictates] (Reveals how ideology dictates policy, such as Manifest Destiny framing "Indian Policy")

(CulturalConcept)-->(MoralValue) [derived_from] (Links abstract culture to its psychological roots, such as Self-reliance deriving from the Liberty/Oppression foundation)