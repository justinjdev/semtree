## ADDED Requirements

### Requirement: Per-level difficulty estimation
The router SHALL compute a difficulty parameter alpha_l at each level during descent, defined as alpha_l = B_l * m_l, where B_l is the branching factor (number of children) and m_l is the ambiguity measure.

#### Scenario: Difficulty computed at each level
- **WHEN** the router descends through a level with 20 children and an ambiguity score of 0.7
- **THEN** the difficulty parameter for that level is 20 * 0.7 = 14.0

#### Scenario: Single-child level has minimal difficulty
- **WHEN** a level has 1 child
- **THEN** the difficulty parameter alpha_l is at most 1.0 regardless of ambiguity

### Requirement: Ambiguity measure from similarity spread
The router SHALL compute ambiguity m_l from the cosine similarity score distribution among siblings at each level. Ambiguity SHALL be defined as m_l = 1.0 - IQR(scores), clamped to [0.1, 1.0], where IQR is the interquartile range (Q75 - Q25) of similarity scores.

#### Scenario: Clustered scores produce high ambiguity
- **WHEN** children at a level have similarity scores [0.81, 0.80, 0.79, 0.78, 0.77]
- **THEN** the IQR is small and m_l is close to 1.0 (high ambiguity)

#### Scenario: Spread scores produce low ambiguity
- **WHEN** children at a level have similarity scores [0.95, 0.70, 0.40, 0.20, 0.10]
- **THEN** the IQR is large and m_l is close to 0.1 (low ambiguity, easy to distinguish)

#### Scenario: Fewer than 4 children
- **WHEN** a level has fewer than 4 children (insufficient for IQR)
- **THEN** the router SHALL use m_l = 0.5 as a default ambiguity

### Requirement: Water-filling beam allocation
The router SHALL allocate beam width b_l at each level proportional to alpha_l, constrained by a total beam budget. The total budget SHALL be beam_width * max_depth. At each level, the allocated beam SHALL be b_l = max(1, round(B_remaining * alpha_l / sum_alpha_remaining)), where B_remaining is the unspent budget and sum_alpha_remaining includes a lookahead estimate for unseen levels.

#### Scenario: Hard level gets wider beam
- **WHEN** level 1 has alpha_l = 20.0 and level 2 has alpha_l = 5.0
- **THEN** level 1 receives approximately 4x the beam width of level 2

#### Scenario: Every level gets at least beam width 1
- **WHEN** a level's proportional allocation rounds to 0
- **THEN** the router allocates a beam width of 1 for that level

#### Scenario: Budget is fully distributed
- **WHEN** the router completes descent through all levels
- **THEN** the sum of allocated beam widths across levels does not exceed beam_width * actual_depth

### Requirement: Lookahead budget reservation
The router SHALL reserve a portion of the beam budget for unseen deeper levels rather than spending the entire budget at shallow levels. The lookahead estimate SHALL assume remaining levels have average difficulty based on levels seen so far.

#### Scenario: Budget reserved for deeper levels
- **WHEN** the router is at level 1 of a max_depth=5 tree with total budget 15
- **THEN** the router does not allocate more than approximately 1/5 of the total budget to level 1 (adjusted by alpha_l proportions)

#### Scenario: Shallow tree uses full budget
- **WHEN** the tree has only 2 levels and max_depth is 10
- **THEN** the remaining budget after level 2 is not wasted; levels 1 and 2 share the full budget

### Requirement: RouteLevel diagnostics for water-filling
When using the waterfill beam policy, each RouteLevel in the output SHALL include diagnostic fields: branching_factor (B_l), ambiguity (m_l), and allocated_beam (b_l).

#### Scenario: Diagnostics present under waterfill policy
- **WHEN** the route command is run with `--beam-policy waterfill`
- **THEN** each level in the output includes branching_factor, ambiguity, and allocated_beam fields

#### Scenario: Diagnostics absent under uniform policy
- **WHEN** the route command is run with `--beam-policy uniform`
- **THEN** the output does not include branching_factor, ambiguity, or allocated_beam fields (or they are null)
