# Caelum

Rust + Slint rewrite of the Caelestia KDE shell. Design:
`docs/superpowers/specs/2026-11-27-caelum-design.md`. Plan:
`docs/superpowers/plans/2026-11-27-caelum/` (start with `00-overview.md`).

## Switching models mid-phase

The plan assigns models (see "Which Claude model to build with" in
`00-overview.md`): Opus for Phase 0, bugs that survive two attempts, and
expanding files 06/07; Sonnet for normal tasks; Haiku for mechanical chores.

When the work in progress needs a different model than the one running (for
example, a Sonnet session hits a bug that has survived two fix attempts, or
reaches a task the plan marks for Opus), **stop. Don't push on with the wrong
model.** Then:

1. Commit any finished, passing work. Never commit failing or half-written code.
   Leave it uncommitted and mention it in the note.
2. Write `docs/handoff.md`, replacing any previous one:

   ```markdown
   # Handoff: <date>

   **Switch to:** <Opus | Sonnet | Haiku>, because <one line>
   **Plan file / task / step:** <e.g. 03-phase1-bar.md / 03-4 / Step 3>
   **Done:** <what is finished and committed, with short SHAs>
   **Uncommitted:** <files changed but not committed, or "none">
   **Blocked on:** <the exact problem; error text, failing test name>
   **Tried:** <each attempt and why it failed>
   **Start the next session with:**
   > Read CLAUDE.md and docs/handoff.md, then continue <plan file> task <N> step <M>.
   ```

3. Tell the user, in one short message: which model to switch to and why, and
   to run `/clear`, then `/model <name>`, then paste the "Start the next
   session with" prompt.

A session that starts from a handoff reads `docs/handoff.md` first. Once it
has taken over, it deletes the file in its first commit.

## Progress

`docs/progress.md` tracks tasks done. When a task's final commit lands, tick
it there with that commit's short SHA, in a follow-up commit (a commit cannot
contain its own SHA).
