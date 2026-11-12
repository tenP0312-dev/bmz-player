# BMS-IR fork notes

This fork (`tenP0312-dev/bmz-player`) is a BMS-IR operator's working copy of
bmz-player. It is not the upstream project and is not released by upstream.

Tracking issue: BMS-Mania/IR#1580. The specs in this folder are copies of the
BMS-IR repository's `docs/bmz/` specs, kept here so work in this fork can read
them without access to the BMS-IR repository.

## Operator decisions (2026-11-12)

1. **Connect to BMS-IR only.** Builds from this fork must not send anything to
   any IR or online service other than BMS-IR (no BMZ official IR, no rianIR,
   no other provider, no upstream online feature that sends player data). A bug
   in this fork must not affect other services. The first change in this fork
   enforces this; every later branch is based on it.
2. FHS mark (`SPEC_FHS_MARK.md`): "used" means Floating was active at least
   once during play. It is sent by the BMS-IR provider (with decision 1, BMS-IR
   is the only destination anyway).
3. READY first notes (`SPEC_READY_FIRST_NOTES.md`) and one-bass input
   (`SPEC_ONE_BASS_INPUT.md`): default OFF. READY display covers the READY
   phase only for now; the loading phase is a later step. One-bass is disabled
   in courses/Dan, G-BATTLE, Random Trainer and key-count conversion. A longer
   reload is accepted.
4. Order: BMS-IR-only restriction, then FHS mark, then READY first notes, then
   one-bass input.
5. One branch `claude/<feature>` and one PR per feature into this fork's
   `main`. PRs are left open (not merged). Nothing is sent to upstream
   (hyrorre/bmz-player): no PR, issue or comment.
6. Do not copy Arena oraja (beatoraja-derived, GPL) code; implement from the
   described behavior. Commit messages do not include AI model names (operator
   decision; overrides the upstream AGENTS.md request).

## Arena foundation decisions (2026-11-12, BMS-Mania/IR#1588)

Spec: `SPEC_ARENA_FOUNDATION.md` (copy of the BMS-IR document). All nine
proposals in the spec are accepted:

1. Start with unrated rooms; rated matches come later (step 7).
2. Server flavor name: `bmz-player`.
3. Separate version/build allowlists per flavor.
4. For now BMZ plays only against BMZ (no mixing with Arena oraja).
5. Fork builds are made on the operator's PC for testing only; not shipped via
   the BMS-IR launcher. Version string like `0.5.1-bmsir.1`.
6. Arena connection is OFF by default.
7. When `replaced` by another client, do not reconnect automatically.
8. During the foundation, the server refuses BMZ in rated play and unsupported
   room settings.
9. 7KEY only at first.
