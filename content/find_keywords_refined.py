import os
import re
import json

keywords_to_retry = [
    ("The March On Washington", r"March On Washington"),
    ("The New Frontier", r"New Frontier"),
    ("The Great Society", r"Great Society"),
    ("Civil Rights Act 1964", r"Civil Rights Act (of )?1964"),
    ("Long Hot Summers", r"Long Hot Summers"),
    ("Malcolm X", r"Malcolm X"),
    ("The New Left", r"New Left"),
    ("The Counter Culture", r"Counter[- ]?Culture"),
    ("Woodstock 1969", r"Woodstock"),
    ("Neil Armstrong", r"Neil Armstrong"),
    ("Cezar Chavez", r"Cesar Chavez"),
    ("Arab Oil Embargo", r"Arab Oil Embargo"),
    ("The Energy Crisis", r"Energy Crisis"),
    ("Jimmy Carter", r"Jimmy Carter"),
    ("SALT II", r"SALT II"),
    ("Iranian Hostage Crisis", r"Iranian Hostage Crisis"),
    ("Camp David Accords", r"Camp David Accords"),
    ("The Me Decade", r"Me [Dd]ecade"),
    ("Ronald Reagan", r"Ronald Reagan"),
    ("NOW", r"\bNOW\b"), # Case sensitive whole word
    ("ERA", r"\bERA\b")  # Case sensitive whole word
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

for kw_label, kw_pattern in keywords_to_retry:
    results[kw_label] = {"exists": os.path.exists(f"{kw_label}.md"), "mentions": []}
    
    # Check if the name variation exists too
    if kw_label == "Cezar Chavez" and os.path.exists("Cesar Chavez.md"):
        results[kw_label]["exists"] = True

    flags = re.IGNORECASE
    if kw_label in ["NOW", "ERA"]:
        flags = 0 # Case sensitive

    for filename in target_files:
        if not os.path.exists(filename):
            continue
        with open(filename, 'r') as f:
            content = f.read()
            matches = re.finditer(kw_pattern, content, flags)
            for match in matches:
                start = max(0, match.start() - 100)
                end = min(len(content), match.end() + 100)
                context = content[start:end].strip()
                
                is_linked = False
                if match.start() >= 2 and content[match.start()-2:match.start()] == "[[" :
                     is_linked = True
                
                results[kw_label]["mentions"].append({
                    "file": filename,
                    "line_context": context,
                    "is_linked": is_linked,
                    "start": match.start(),
                    "end": match.end(),
                    "matched_text": match.group(0)
                })

print(json.dumps(results, indent=2))
