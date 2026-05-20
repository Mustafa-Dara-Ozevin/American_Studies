import os
import re

keywords = [
    "Conservatism", "Conformism", "Consumerism", "Julius & Rosenberg",
    "Brown vs Board of Education of Topeka 1954", "Crisis in the Little Rock, Arkansas 1957",
    "MLK Jr", "Montgomery Bus Boycott 1955", "The Sit-Ins", "Baby Boom",
    "Affluent Society", "GI Bill", "Beat Generation", "McCarthyism", "Hiss Trial",
    "Bay of Pigs", "Cuban Missile Crisis", "Geneva Accords", "Vietminh",
    "Ngo Dinh Diem", "Tonkin Gulf incident", "Tet offensive 1968", "My Lai",
    "Pentagon Papers", "Nixon Doctrine", "War Powers Act", "Paris Peace Agreement",
    "Detente", "SALT Talks", "SAlT Talks", "Eisenhower Doctrine", "Suez Crisis",
    "Civil Rights Movement", "The March On Washington", "The New Frontier",
    "The Great Society", "LB Johnson", "Civil Rights Act 1964", "Long Hot Summers",
    "Malcolm X", "Black Power", "The New Left", "The Counter Culture",
    "Woodstock 1969", "Greenwich Village", "The Feminine Mystique", "NOW", "ERA",
    "Neil Armstrong", "Watergate Scandal", "Nixon", "Cezar Chavez",
    "Arab Oil Embargo", "The Energy Crisis", "Jimmy Carter", "SALT II",
    "Iranian Hostage Crisis", "Camp David Accords", "The Me Decade", "Ronald Reagan"
]

target_files = [
    "Americans in the Great War.md", "Interbellum in Europe.md", "World War 2.md",
    "World War 2 chapter 2.md", "Korean War.md", "1945-1960 America at the Midcentury.md",
    "1960-1968 The tumultous sixties.md", "Vietnam War 1955-1975.md",
    "A Pivotal Area 1969-1980.md", "F.D Roosevelt.md", "E. Roosevelt.md",
    "first hundred days.md", "New Deal 1.md", "New Deal 2.md",
    "Second Hundred Days.md", "Brain trust.md", "Social Security Act.md",
    "Wagner Act.md", "Revenue Act.md", "Dust Bowl.md", "Memorial Day Massacre.md",
    "Scottsboro trials.md", "Sit Down Strikes.md", "League of Nations.md",
    "Paris Peace Conference.md", "Trench Warfare.md", "Triple Alliance.md",
    "Triple Entrente.md", "Nixon.md"
]

results = {}

for kw in keywords:
    results[kw] = {"exists": os.path.exists(f"{kw}.md"), "mentions": []}
    # Also check with alias or simplified name if necessary, but sticking to literal for now
    
    for filename in target_files:
        if not os.path.exists(filename):
            continue
        with open(filename, 'r') as f:
            content = f.read()
            # Find literal mentions (not inside brackets already)
            # This regex is a bit simple, but let's see.
            # We want to find "Keyword" but NOT "[[Keyword]]"
            # Actually, the user says "turn that mention into hyperlink", so finding unlinked ones is priority.
            # But we also need context for ALL mentions to populate the new note.
            
            # Find all occurrences to gather info
            matches = re.finditer(re.escape(kw), content, re.IGNORECASE)
            for match in matches:
                start = max(0, match.start() - 100)
                end = min(len(content), match.end() + 100)
                context = content[start:end].strip()
                
                # Check if it's already linked at this position
                is_linked = False
                if match.start() >= 2 and content[match.start()-2:match.start()] == "[[" :
                     is_linked = True
                
                results[kw]["mentions"].append({
                    "file": filename,
                    "line_context": context,
                    "is_linked": is_linked,
                    "start": match.start(),
                    "end": match.end()
                })

import json
print(json.dumps(results, indent=2))
