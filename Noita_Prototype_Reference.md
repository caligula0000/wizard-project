# Noita engine reference for material physics prototypes

Language agnostic research and implementation guide
Prepared 25 October 2026

## Purpose and boundaries

Build a bounded, turn-based artillery arena with granular materials and destructible solid objects. The defining experiment is a wooden support that burns or is blasted through, releasing a solid section that falls, rotates, and affects the battlefield. World streaming, procedural biomes, wand construction, and extensive chemistry are outside the first prototypes.

The recommended foundation is a fixed material grid, event-driven activity tracking, explicit structural connectivity, and a conventional rigid-body solver. Start with serial simulation and a small material set. Prove the interaction between detached bodies and the grid before expanding content.

## How to read this document

Documented findings are collected on the next page and tied to numbered sources. The implementation sections are proposed engineering choices for this project, not a reconstruction of Noita source code. Named algorithms in the reading guide are useful investigation targets; details not established by accessible primary text are explicitly marked.

Coverage is a curated compilation of the relevant public material located in this research, not a claim to have found every interview or watched every recording. Several recordings expose only metadata to retrieval. Their content is not represented as fully audited. Most detailed developer explanations date from 2019 and describe that version of the engine.

## First prototype target

- One arena; bedrock, wood, sand, water, and fire; a projectile and a simple explosion.

- A supported wooden bridge above sand and water. Burning or severing its support releases a rotating fragment.

- A debug view for active cells, connected regions, body outlines, and material counts.

- Repeatable input scripts and a fixed simulation timestep. No networking requirement in the first milestone.

## What the public sources establish

## Material simulation and local updates

Developer interview [2]: materials follow local movement and reaction rules. Liquids descend or spread sideways; density comparisons permit swaps. Gases rise. Burning wood eventually loses pixels. Updates proceed bottom-up for falling materials. The simulation uses 64 × 64 regions with dirty rectangles. A per-frame marker avoids repeat updates. Parallel work uses four checker-pattern passes with bounded access outside each region.

## Rigid objects and collapse

Developer interview [2]: body pixels retain their owning body and local positions. Removing pixels triggers shape reconstruction and can create several bodies. Terrain collapse is selective: suitable material becomes falling grains or is carved into rigid chunks. This does not establish a universal structural-integrity model.

## CPU and world organisation

Purho’s developer reply [3] identifies CPU simulation and polygon triangulation for Box2D. It describes 512 × 512 world chunks for generation and storage. These are a different level of organisation from the 64 × 64 simulation regions; neither size is a requirement for a bounded arena. The reply also describes tracking update areas beyond the visible screen.

The developer’s IGF interview [4] confirms Box2D as the rigid-body library. The official FAQ [5] describes material behaviour and simplified chemistry, electricity, and thermodynamics. These are useful scope references, not complete implementation specifications.

## Evidence limits that affect implementation

- The GDC talk [1] is the principal engine presentation. The familiar outline extraction → simplification → triangulation pipeline is a reading target; complete transcript and frame-by-frame verification were unavailable in this research.

- Exact body-to-fluid impulse coupling, collider rebuild cadence, contact generation against granular material, and rotation rasterisation policies remain unspecified here.

- The proposed body overlay and support algorithm below are design decisions, not confirmed Noita internals.

- A local edit can have nonlocal structural effects. A dirty rectangle bounds material work; it does not necessarily bound a support search.

## Data model and simulation ownership

Proposal. Keep the world authoritative in a small number of explicit data stores. Use integer grid coordinates for materials and continuous coordinates for rigid-body transforms. Define one conversion between grid cells and physics units; avoid assuming a rendered pixel is a physics unit.

## Core records

- Material definition: movement class, density ordering, flammability, burn duration, blast resistance, and structural category. Add thermal properties only when needed.

- Grid cell: material identifier, small state payload such as burn timer, and update stamp. Temperature is optional. Empty space is explicit.

- Simulation region: current activity bounds, next activity bounds, and separate flags for topology, collision geometry, and rendering changes.

- Rigid body: stable identifier, local material mask, position and rotation, linear and angular velocity, and collision geometry derived from its mask.

- Transient occupancy: world cells covered by moving bodies, with references back to body-local cells. This is a query structure, not another permanent copy of the material.

- Events: cell changed, ignition scheduled, structural edit, body fracture, and impulse request. Batch events before expensive reconstruction.

## Ownership invariants

Every material unit belongs to either the static grid or one body-local mask. Moving a region into a body transfers ownership atomically. A rasterised body overlay must never be counted as additional material. Rebuilding geometry must not reset fire, damage, or other cell state.

The collision representation is derived data. Preserve the material mask when a polygon approximation loses a notch or corner. Keep structural connectivity in the mask, so simplification cannot accidentally reconnect two severed pieces.

## First memory estimate

For a proposed 1024 × 512 arena, a cell store using 8 bytes per cell consumes 4 MiB. A separate 4-byte ownership map adds 2 MiB. Body masks, collision shapes, queues, and render buffers are additional. This is an illustrative budget, not a Noita measurement or a promised performance target.

## Separate sleep decisions

A body can be mechanically asleep while its wood continues burning. A region can be visually unchanged while a timer or reaction is pending. Maintain independent scheduling for motion, reactions, structure, and rendering.

## Local material updates without streaming

Proposal. Allocate the complete bounded arena. Begin with one active flag per region, then introduce dirty rectangles if profiling shows a benefit. A 32 × 32 or 64 × 64 region is a tuning candidate, not a correctness requirement.

## Activity scheduling

Maintain two activity sets. Read the current set while writing the next set; do not clear newly awakened work while iterating. A cell edit wakes the source, destination, and the neighbourhood whose rules depend on them, including across region borders. Moving bodies wake their swept area, not only their final bounds.

A settled region can sleep only when no movement is possible and no continuing process requires service. Schedule future fire and reaction events, or retain those cells in an active reaction list. Failed movement alone is not proof that a burning cell is idle.

## Movement rules for the first prototype

- Sand: try down, then a permitted downward diagonal. Vary left/right preference with a seeded rule to reduce directional bias.

- Water: try down, then bounded lateral movement. Density swaps require explicit compatibility rules; a single numeric comparison should not make every material pass through every other material.

- Wood and bedrock: immobile in the grid. Wood participates in ignition and destruction; bedrock supplies explicit anchors.

- Fire: attach combustion state to fuel cells, schedule fuel consumption, and attempt bounded neighbour ignition. Add visible flame particles separately if useful.

Process downward-moving material bottom-up. Define gas scheduling separately if gases are added. When cells move or swap, preserve their state and mark the affected participants consistently so a cell does not move several times simply because its destination is visited later.

## Serial reference before parallel work

A serial implementation is the correctness baseline. Do not assume processing arbitrary chunks preserves a global bottom-up order at their boundaries; compare seams against a full-grid reference. Noita’s four-pass method is a later reference [2], not something to copy without proving all read and write neighbourhoods are safe. Explosions and long-range effects should be queued outside bounded local passes.

## Useful counters

Record visited cells, changed cells, awake regions, scheduled reactions, and time spent in material updates. A static scene should approach negligible material work, while an edit should reliably awaken every dependent region.

## Support detection and fracture

Proposal. Use connectivity to explicit anchors as the first collapse rule. This gives predictable detachment without simulating stress or bending. A thin connected bridge remains supported until it is actually severed; that limitation should be visible in the design.

## Keep structural connections distinct from contact

Touching is not necessarily being welded. Give authored structures an identity or explicit bonds. A wooden beam resting on stone should be a body supported by collision contacts, or have a deliberately authored attachment. If all adjacent solid cells are treated as bonded, unrelated rubble can accidentally glue itself to terrain.

## After a structural edit

- Batch removals and identify the affected structure or terrain component.

- Flood-fill the remaining structural cells using a chosen adjacency rule. Four-neighbour connectivity is a simple first choice; decide explicitly whether diagonal contacts count.

- For each component, determine whether a permitted connection reaches an anchor. Search through the actual component, including beyond the edited region.

- Keep anchored components static. Transfer disconnected components to body-local masks, build collision shapes, and wake the surrounding simulation.

- Apply a declared small-fragment policy: keep a body, convert to grains, or remove with an explicit gameplay rule. Track the conversion in material accounting.

## Local updates do not guarantee local connectivity

Cutting one cell can release a long bridge. For the first bounded arena, a complete search of the affected structure is often the simplest correct option. Instrument its cost. Later, cache components or process large searches over several ticks. Until a search finishes, retain a coherent prior state instead of releasing partially classified material.

## Fracturing a moving object

Run the same component search in the body-local mask. Rebuild once per body per tick after all edits. Give each fragment its inherited orientation and angular velocity. Its initial linear velocity should include the parent’s rotation at the fragment centre: parent linear velocity plus angular velocity crossed with the centre offset. This avoids fragments abruptly losing their existing motion.

## Sleeping and settling

Keep landed pieces as sleeping bodies initially. Baking them back into terrain is optional and changes bonding semantics, pixel ownership, and later fracture behaviour. Avoid introducing that conversion until the basic interaction is stable.

## Collision geometry and the material bridge

Proposal. Build a replaceable adapter between material masks and the rigid-body solver. Box2D demonstrates the approach in Noita [3, 4], but the architecture should require capabilities rather than a particular language or library.

## From a mask to collision shapes

Extract boundary loops, preserve holes, simplify within a chosen error tolerance, and decompose the result into shapes the solver accepts. Marching squares, Douglas–Peucker simplification, and triangulation are candidate algorithms associated with the GDC reading path [1]. Treat topology preservation as a separate requirement: small holes and narrow necks can be destroyed by careless simplification.

Use an initial error tolerance below the thickness of the smallest gameplay-relevant feature, then tune through tests. Reject degenerate polygons, inconsistent winding, self-intersections, and sliver triangles. The solver should see multiple convex pieces attached to one body when required. General collision-shape constraints are described in [9].

## Terrain collision

Generate static collision geometry for occupied solid terrain near bodies, or cache it by region. Rebuild dirty geometry before the next relevant physics step. Include neighbouring borders when an edit changes a seam. Ensure boundaries do not create internal edges that snag bodies or leave cracks. Sand and water need an explicit policy; do not automatically turn every moving grain into a rigid-body collider.

## Body occupancy in the material grid

Keep the body-local mask authoritative and derive world occupancy from its transform. Continuous rotation does not map one-to-one onto integer cells: several source pixels can cover one destination, and gaps can appear. Use a consistent coverage rule, stable conflict resolution, and tests for holes and duplicate mass. Never overwrite underlying material silently.

Begin with solid bodies blocking material movement. Add displacement by relocating overlapping liquid or grains into nearby valid cells. If no destination exists, retain the displaced material in an explicit pending store or constrain movement according to the prototype policy. Do not let a failed placement destroy water accidentally.

## Coupling levels

First prove body–terrain and body–body collision. Next prove sand and water cannot leak through body masks. Then add approximate drag or buoyancy if gameplay requires it. Fully bidirectional granular forces and pressure are a separate scope increase. A large body resting on sand is an important acceptance case; an occupancy overlay alone will not provide that support.

## Tick order and turn resolution

Proposal. Use a fixed simulation timestep and a documented phase order. The sequence below is a starting contract, not a claim about Noita’s loop. Resolve ownership changes at phase boundaries and retain a serial reference implementation.

## Suggested tick contract

- 1. Consume commands for this tick. Advance projectiles with swept collision queries and queue impacts.

- 2. Apply impact edits and scheduled material reactions. Record topology and collider changes.

- 3. Classify structural components, split affected bodies, and rebuild required collision geometry.

- 4. Apply impulses and step rigid-body physics against the updated solid terrain.

- 5. Rebuild transient body occupancy. Resolve newly overlapping loose material using the declared displacement policy.

- 6. Update active granular and liquid cells against the current occupancy. Queue secondary reactions and structural edits for the next tick.

- 7. Commit next activity sets, render changes, and diagnostic counters. Evaluate turn resolution.

This introduces up to one tick of delay for secondary structural changes. If that becomes noticeable, use bounded substeps or an additional controlled reconstruction stage. Do not recursively process reactions without a limit; a chain explosion can otherwise monopolise a frame.

## Explosions

Start with a circular edit plus impulses applied to nearby bodies. Use different material resistance thresholds. This deliberately simple model may damage through barriers; add ray-based attenuation or occlusion only when needed. Noita’s exact explosion implementation is not specified by the primary evidence compiled here.

## Turn policy

For an initial experiment, pause simulation while aiming, run it during the shot, and allow a short settling window afterward. Require no active projectile and low body motion for a configured interval, but impose a maximum resolution duration. Pause lingering fire and liquids with the rest of the simulation until the next action. These rules are game-design choices and should be exposed as settings.

## Replay and future multiplayer

Record initial state, material definitions, seed, commands, and tick indices. Seeded randomness alone does not guarantee cross-platform determinism: floating-point physics, iteration order, and scheduling can diverge. A deterministic local replay is a useful debugging target; networking architecture can remain deferred.

## Prototype sequence and acceptance scenes

Proposal. Implement in small steps with visible pass criteria. Keep the same fixtures throughout development so local optimisations can be compared with the reference simulation.

## Prototype one establishes the material grid

Build sand, water, bedrock, and empty space with fixed ticks. A sealed tank must retain water; a sand pile must settle without drifting systematically sideways. Compare a pile spanning region boundaries with the same pile inside one region. Count material before and after movement and swaps.

## Prototype two proves the beam interaction

Add wood, fire, anchored structure masks, and component detection. A beam remains attached while a valid connection exists. Burning through its last connection releases the whole detached section, which rotates and collides with the floor. Cutting a moving beam creates independent fragments with inherited motion.

## Prototype three proves material and body coexistence

Place the beam above a water basin and sand pile. Test slow rotation, fast falls, narrow gaps, and a body landing on loose material. Track occupancy conflicts, lost material, penetration, and material trapped inside a mask. Decide how much sand support and liquid displacement the game actually needs.

## Prototype four adds local scheduling

Introduce sleeping regions and dirty rectangles only after the full-grid baseline works. Ignite a stationary beam, remove a support on a chunk border, and cut a bridge extending far outside the edited area. These catch missed timer wake-ups, seam bugs, and incorrectly bounded support searches.

## Prototype five adds the artillery loop

Add two placeholder characters, aiming, a projectile, an explosion, and turn resolution. Test a shot that ignites a support and a shot that breaks it immediately. Tune whether the resulting collapse is readable, tactically useful, and resolved within a satisfying turn duration.

## Measure before choosing optimisations

Collect median and worst-frame times separately for material updates, connectivity searches, shape reconstruction, rigid-body solving, rasterisation, and rendering. Also track active cells, body count, collision piece count, reconstructed area, and pending displacement. Establish a target device and frame budget before interpreting performance.

Worst-case fixtures: a checkerboard of tiny fragments, an arena full of water, many simultaneous fires, a thin bridge across the map, and repeated explosions into already fragmented wood. Optimise the dominant cost rather than assuming cell updates are the bottleneck.

## Source catalogue and reading order

Sources are linked for follow-up. Primary developer statements support the documented findings; the design sections are explicitly proposed. Video listings confirm provenance and subject matter, but full recordings were not audited. No standalone official slide deck or complete engine source release was verified in this research.

### 1  Exploring the Tech and Design of Noita

Petri Purho, GDC 2019. First watch: falling simulation, rigid bodies, static geometry, multithreading, and continuous worlds. Skip world streaming for the first arena. Official recording and session description.

<https://www.youtube.com/watch?v=prXuyMCgbTc>

### 1a  GDC session record

Official abstract and speaker metadata; confirms technical scope.

<https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design>

### 2  Noita a Game Based on Falling Sand Simulation

Developer interview, 80 Level, 5 April 2019. Best accessible written source for local material rules, selective collapse, body-local pixels, dirty rectangles, and four-pass updates.

<https://80.lv/articles/noita-a-game-based-on-falling-sand-simulation>

### 3  Noita pixel simulation discussion

Petri Purho developer replies, September 2019. CPU simulation, storage chunks, off-screen update areas, and triangulation into Box2D. Relevant replies were search-index accessible; full thread retrieval was intermittent.

<https://www.reddit.com/r/gamedev/comments/d93op6/noita_pixel_simulation_any_tip_about_how_is_it/>

### 4  Road to the IGF Nolla Games Noita

Developer interview, Game Developer, March 2019. Engine dependencies and development context. Search-index text confirms Box2D; full-page fetch timed out.

<https://www.gamedeveloper.com/game-platforms/road-to-the-igf-nolla-games-i-noita-i->

### 5  Official Noita FAQ

Material categories and broad simulation scope. Useful terminology, not an engineering specification.

<https://www.noitagame.com/>

### 6  Fantastic Arcade 2018 Holedown and Noita

Earlier presentation and demonstration. Noita segment begins around 58 minutes. Supplementary viewing; not independently transcribed here.

<https://www.youtube.com/watch?v=kZsMZjzT0ZM&t=3500s>

### 7  Making of Noita The Pixel Based Physics Roguelite

Games Now lecture, 2019. Broader development discussion. Listed for completeness; no unverified implementation claim relies on this recording.

<https://www.youtube.com/watch?v=Mr3E_voEG8Y>

## Further references and unresolved questions

### 8  Official release notes

Useful for the history of physics objects and fixes. These confirm behaviour changes, not the algorithms behind them.

<https://www.noitagame.com/release_notes/>

### 9  Box2D collision documentation

Primary library documentation for convex collision shapes and decomposition constraints. Current documentation is not proof of the exact Box2D version or modifications used by Noita.

<https://box2d.org/documentation/md_collision.html>

### 10  Official engine landing page

Nolla’s pointer to the GDC talk.

<https://nollagames.com/fallingeverything/>

### 11  Noita interviews and videos index

Community discovery index for further developer interviews and recordings. Use original developer sources for factual claims.

<https://noita.wiki.gg/wiki/Official_Interviews_and_Videos>

## Questions to resolve by experiment

- Should structures use explicit bonds, anchor-connected terrain, or pre-existing bodies joined to anchors? The beam test should compare these approaches.

- How should a body be supported by a sand pile? Candidate approximations include settled-sand collision proxies or sampled resistance forces, each with different failure modes.

- What rasterisation rule best preserves thin rotating objects without inventing or deleting material?

- How much fluid displacement is needed for readable gameplay before pressure, buoyancy, and drag become worthwhile?

- Should burning continue between turns? Which resolution rule makes delayed collapse tactically understandable?

## Scope to defer

World streaming, procedural generation, universal structural stress, full thermodynamics, advanced fluid pressure, automatic terrain welding, GPU migration, and network lockstep all remain optional. The first successful result is a small arena in which a damaged support releases a coherent falling object and the surrounding materials respond consistently.
