# Checklist Shape Specification

Use this spec when rewriting a checklist into execution-ready form.

## Core principle

A normalized checklist should make four things obvious without extra explanation:
- what the current state is
- what the target state is
- which peer points are parallel
- which steps remain serial inside a single point

## Top-level structure

Prefer this order unless the user requests a lighter format:

1. Goal
2. Constraints
3. Current status snapshot
4. Parallel task summary
5. Per-task sections
6. Not in this round
7. Recommended execution order
8. Overall acceptance line
9. Progress summary template

## Required top-level fields

### Goal
State the intended end condition of the checklist in direct terms.

### Constraints
State hard boundaries and non-goals.
Include limits such as:
- scope boundaries
- forbidden directions
- architectural guardrails
- host/runtime restrictions
- single-writer continuity restrictions when shared state exists

### Current status snapshot
Summarize what is already done, in progress, blocked, or still open.

### Parallel task summary
List the checklist points that can proceed independently.
Treat peer points as parallel by default unless a dependency note says otherwise.
For each point, name the primary goal and the main owned surface.
If a shared continuity refresh is needed, name the dedicated integrator lane explicitly instead of leaving it implicit.

### Recommended execution order
Even when work can run in parallel, specify the preferred sequencing or priorities.

### Overall acceptance line
State the conditions that must all be true before the whole checklist is considered complete.

### Progress summary template
Provide a simple status rollup such as total / done / in progress / not started.

## Required fields for each task point

Every execution-ready point should include:

### Current state
What is true now.

### Goal
The ideal state for this point.

### Scope or exclusive surface
What this point is allowed to own or modify.

### Forbidden scope
What this point must not touch when isolation matters.
For parallel lanes, default forbidden scope should include shared continuity artifacts unless the point is the designated integrator.

### Deliverables
What artifacts, files, or outcomes should exist after completion.

### Acceptance criteria
What can be checked to prove this point is done.
These should be concrete and falsifiable.

### Execution result slot
Reserve a place for post-execution updates such as done / blocked / failed / partial.

## Normalization rules

### Keep serial work inside one point when
- the work shares one goal
- the same owner or write surface should carry it
- the steps must happen in order
- splitting the work would only create artificial peer bullets
- the serial chain is long but still belongs to one bounded execution point

In that case, keep one checklist point and use ordered substeps.

### Treat peer points as parallel by default when
- the checklist has multiple peer points and no dependency note says otherwise
- each point is presented as a separate execution point or lane

Only override this default when the checklist explicitly marks a prerequisite or ordering dependency.

### Split work into parallel points when
- items can progress independently
- different owners or lanes may execute them
- the write surfaces differ
- combining them would hide isolation boundaries

In that case, create separate task points and state their boundaries.
Do not let peer points co-own the same continuity files; add a separate integrator point for the final merge and global artifact flush.

## Missing information policy

If the source material does not provide a required field:
- keep the field in the structure
- mark it as missing or pending clarification
- do not invent specifics

## Anti-patterns

Do not produce these shapes:
- one giant bullet containing unrelated parallel work
- many flat bullets that are actually ordered serial steps
- tasks with no goal or no acceptance criteria
- tasks that can modify anything with no boundary language
- parallel points that all say they may update the same global continuity files
- checklists that say work is complete but provide no place to update execution results
