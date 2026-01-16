# ⚡ PETAL_DRIFT — Clippy

This lint exists to combat minimal change bias.

Clippy warnings often indicate deeper design issues, not just style violations.
Address the underlying design issue, not just the symptom.

## Consider These Refactoring Patterns

1. COMPLEXITY WARNINGS (cognitive_complexity, too_many_arguments)
   These signal functions doing too much.
   Should this function instead compose smaller abstractions?
   Can we extract behavior into methods?
   Weigh the cost of abstraction with the cost of the separation between components.

2. TYPE WARNINGS (type_complexity)
   Complex types suggest missing abstractions. Introduce newtypes or
   domain-specific types to clarify intent.

3. CLONE/COPY WARNINGS
   Excessive cloning may indicate ownership design issues. Consider
   borrowing patterns or restructuring data flow.

