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
    "Detente", "SALT Talks", "Eisenhower Doctrine", "Suez Crisis",
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

unlinked_found = []

for kw in keywords:
    pattern = re.compile(r'(?<!\[\[)' + re.escape(kw) + r'(?!\]\])', re.IGNORECASE)
    # Special cases for variations
    variations = []
    if kw == "The Me Decade": variations.append("Me decade")
    if kw == "Cezar Chavez": variations.append("Cesar Chavez")
    if kw == "The Counter Culture": variations.append("Counterculture")
    if kw == "Paris Peace Agreement": variations.append("ceasefire agreement in Paris")
    if kw == "My Lai": variations.append("My Lai Massacre")

    for v in variations:
        pattern = re.compile(r'(?<!\[\[)' + re.escape(v) + r'(?!\]\])|' + pattern.pattern, re.IGNORECASE)

    for filename in target_files:
        if not os.path.exists(filename): continue
        with open(filename, 'r') as f:
            content = f.read()
            if pattern.search(content):
                unlinked_found.append((kw, filename))

for kw, filename in unlinked_found:
    print(f"Unlinked '{kw}' found in '{filename}'")
