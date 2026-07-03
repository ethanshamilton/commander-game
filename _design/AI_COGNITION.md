# AI Cognition & Command — Design Exploration

Status: **exploratory, not ready for implementation.** This captures a design
conversation about the mental model for unit AI. The goal is a shared
vocabulary and a set of load-bearing concepts to design against.

## Context

The truth pipeline (see `docs/truth_pipeline.md`) already separates ground
truth → unit-local memory → player knowledge. The AI question is: what lives
inside "unit-local memory → decision," and how does the chain of command
distribute agency?

## The central structure: the goal/plan tree

Most open questions (what is an order, what is intent, what is the smallest
decision, when to disobey, what to do on comms loss) resolve against **one
missing structure**: a goal hierarchy / plan tree that spans the entire chain
of command.

- A **goal** is a desired world-state ("ridge held by friendlies").
- A **plan** decomposes a goal into subgoals/tasks.
- An **action** is a leaf — atomic, executable this tick (move, fire, send
  packet).
- An **order is a communicated goal with authority attached** — a social fact
  that installs a goal into a subordinate's hierarchy.
- **Intent is a pointer one level up**: the parent goal in the *superior's*
  plan tree. "Take that ridge" (my goal) *because* "cover Bravo's flank"
  (parent goal). Intent is not a separate representation; it is the edge above
  the order in a tree that spans the whole command hierarchy.

Payoffs:

- **Autonomy under comms loss**: when an order becomes impossible or stale, a
  unit with intent attached can re-plan against the parent goal ("can't take
  the ridge; can I cover Bravo's flank from the treeline?"). Without intent,
  fallback is expire → request update / rejoin comms. Both behaviors, one
  mechanism.
- **"Smallest unit of decision" dissolves**: a decision is selecting a
  decomposition or action for a goal. Decisions happen at every level of the
  tree, which is why the question felt unanswerable as posed.

### Order grammar

Split the components rather than treating orders as monolithic:

- "Take that ridge" — a **goal**
- "Avoid this area" — a **constraint**
- "Disengage at 30% casualties" — a **termination trigger**

Order ≈ goal + constraints + triggers + intent-pointer. This mirrors the
military OPORD format (SMEAC: Situation, Mission, Execution w/ commander's
intent, Sustainment, Command & Signal). The ontology is pre-solved; read a
real OPORD template.

### Expiration

Order expiration (v1 staleness solution) doubles as an implicit **heartbeat**:
"if you haven't heard from me by then, assume the picture changed." Note that
expiration handles staleness but not *impossibility* — a validity check ("can
this goal still be pursued?") comes later and also falls out of the plan tree.

## The cognitive environment

Working definition: *the typed set of representations an agent can decide
over*. Three types, with different update/reconciliation semantics:

1. **World-beliefs** — contacts, terrain, own position. Sourced from
   perception + comms. Reconciled **evidentially** (recency, source
   reliability).
2. **Social facts** — orders, rank, who my superior is, ROE. Institutional:
   they exist because someone with authority said so. Reconciled by
   **authority** (rank, chain of command).
3. **Self-state** — ammo, health, current task.

Different epistemology per type. This explains why contact reports reconcile
by recency but orders reconcile by rank: it's a consequence of the type
system, not an ad hoc rule.

(Aside: "cognitive environment" is a term of art in Sperber & Wilson's
relevance theory — the set of facts manifest to an agent. Compatible usage.)

### Order conflicts and the command forest

Doctrine has *unity of command*: you take orders from exactly one direct
superior, precisely so order-reconciliation is structurally unnecessary.
Decision: orders flow through a strict **command tree/forest**, so conflicts
only arise in edge cases (superior died, unit reassigned). Rank+recency is
the fallback rule for those edges, not the normal path.

## The two-layer motivation architecture

"Survive → regroup → rejoin comms" on leader death is not decided in the
moment — it's **doctrine**, a standing contingency carried by every soldier.
Implied architecture:

- A stack of **standing goals** (survive, maintain comms, obey current order)
  whose relative weights shift with context.
- The **current order slots into that stack** rather than replacing it.

Open seam: **when does an order outrank survival?** That gap between "obey"
and "survive" is where morale, suppression, and discipline live later. Not
modeled yet, but the seam is deliberate.

### Societies (long-term vision)

Playable factions distinguished by **different individual decision-making
profiles**, hence different optimal strategies. The standing-goal weights are
the natural knob:

- *Authoritarian theocracy*: units weight mission over survival (will make
  sacrifices) but are inflexible under changing conditions — low willingness
  to re-plan against intent, high adherence to the literal order.
- *Individualist society*: highly autonomous units (aggressive re-planning
  against intent, good comms-loss behavior) but won't make major sacrifices.

Society = a parameterization of the standing-goal stack + re-planning
policy. This only works if the two-layer architecture and the goal tree exist
first.

## Rationality and failure aesthetic

Position: **units are perfect rational actors within their cognitive
environment.** This is bounded rationality with all the boundedness located
in the *inputs* (impoverished cognitive environment), not the *inference*.

Consequences:

- Failures are **tragic rather than stupid** — the unit did the right thing
  given what it knew; drama comes from what it didn't know. Every bad outcome
  becomes a story about the comms/intel systems, i.e., the core mechanics.
- Caveats: (a) true optimality is computationally unaffordable; in practice
  "rational" means *satisficing with no legibly-dumb moves*. (b) This makes
  the perception/comms model load-bearing for all difficulty and drama —
  probably the correct weight, but eyes open.
- Any actual dumb behavior is by definition a bug, not a feature.

## Legibility via tracing

Design invariant: **no action without a citable reason.** Every action
carries a justification triple — *(belief, goal, rule)*. The trace is not
instrumentation bolted on; it *is* the decision record.

Triple-purposed:

1. **Debugging** now — visualize per-unit log of intel, orders, actions.
2. **Eval substrate** later — assertions over traces in headless sims
   ("assert no unit engaged without a contact belief").
3. **Player UX** eventually — after-action reports, "why did Alpha retreat?"

## Confidence vs. posture

Two distinct things that interact but aren't identical:

1. **Confidence in a belief** — "how sure am I there's a contact there?"
   V1: scalar decay over time.
2. **Risk posture of behavior** — "how aggressively do I act?" High-confidence
   intel can justify either aggression (known weak enemy) or caution (known
   strong enemy).

Need a system that maps confidence (+ other context) → posture; they are not
the same variable. Also note: scalar decay is a lossy stand-in for the real
structure of stale positional intel — "somewhere in an expanding region
centered on last known position." The region form is more decision-relevant
(can I flank it? does it threaten my route?). Scalar decay is the right v1;
know what it approximates.

## Squad coordination

If coordination happens *only through the comms graph*, squad cohesion is an
emergent property of the epistemic pipeline — no special squad-brain system.
The leader is just the node whose outbound packets carry authority.

Implication to embrace: "the squad believes X" is always an approximation —
squads can hold *incoherent* collective states mid-sync. Not a bug; a drama
engine.

Deferred: units modeling other units' beliefs (theory of mind). Complexity
cliff; revisit much later.

## The player is a node in the command graph

Player-as-mission-assigner + symmetric enemy AI, combined: the player is a
commander whose UI is a comms terminal — an *inhabitant* of the simulation,
not an exception to it. Corollaries:

- An AI can occupy the player's rank → self-play testing, difficulty via
  commander quality, eventually "playable at any level of the hierarchy."
- Fog of war is literally "what has been reported to your node," not a
  rendering trick.

Promote to explicit design principle alongside the truth pipeline.

## Open questions

- **Ground state of autonomy**: what does a unit do with no valid order and
  no reachable superior? (Doctrine? Last known intent? Self-preservation?)
  The answer reveals what agents fundamentally *are* when nobody's telling
  them anything.
- When does an order outrank survival? (Morale/discipline seam.)
- Does a squad have any shared state, or is all apparent squad-level behavior
  N individually-held beliefs synced over comms? (Leaning: the latter.)
- How does confidence + context map to posture?

## Study list

- **HTN planning** (hierarchical task networks) — the plan tree with
  machinery attached; natural fit because the chain of command *is* a task
  hierarchy.
- **Mission command / Auftragstaktik, commander's intent** — the plan tree as
  practiced by actual armies; battle drills as pre-compiled doctrine.
- **OPORD / SMEAC format** — pre-solved order ontology.
- Later: utility AI, belief representation under partial observability
  (occupancy/influence maps), blackboard coordination, legibility literature
  (HRI intention legibility).
