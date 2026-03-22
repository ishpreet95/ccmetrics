# Evaluations

Quality gate records for cc-metrics. Each subfolder contains evaluation artifacts scored against defined frameworks.

## Structure

```
evaluations/
  prd/                    # PRD scoring against 100-point AI-optimization framework
  user-stories/           # Individual user story evaluation (completeness, testability)
  code/                   # Code review evaluations (post-implementation)
  launch/                 # Launch readiness checklist evaluations
```

## Scoring Framework

PRD evaluations use a 100-point framework across 4 categories:

| Category | Max | What it measures |
|---|---|---|
| A. AI-Specific Optimization | 25 | Phase structure, non-goals, format for AI execution |
| B. Traditional PRD Core | 25 | Problem statement, goals, personas, technical specs |
| C. Implementation Clarity | 30 | Requirements, NFRs, architecture, phasing |
| D. Completeness | 20 | Risks, dependencies, examples, documentation quality |

Grade scale: A+ (90-100), A (80-89), B (70-79), C (60-69), D (<60)

## Convention

Files are named: `YYYY-MM-DD-<what>-<version>.md`
