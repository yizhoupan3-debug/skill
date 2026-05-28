# REVIEW_GATE env matrix fixtures

| id | MODE | FORK infer env | cap | notes |
|----|------|----------------|-----|-------|
| em-01 | strict | on | default | multiset baseline |
| em-02 | lite | on | default | id pending vec |
| em-03 | lite | off | default | fallback strict |
| em-04 | strict | off | 2 | cap refused |
| em-05 | lite | on | 2 | lite id AtCap + cap_refused |
| em-06 | strict | on | default | parallel ids |
