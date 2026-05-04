# 0019: spec migration bulk frontmatter strategy and project-specific exceptions

## Status

Accepted (2026-05-05)

## Context

The global `~/.claude/CLAUDE.md` was overhauled (merged via `std-koh-hinooka#10`) to introduce:

- A flattened `docs/{specs,plans,adr}` layout (the legacy `docs/superpowers/` directory is retired)
- A 7-field YAML frontmatter for every spec
- Glossary canonical placement under `~/Obsidian/<scope>/glossary/<concept>.md` (the legacy `docs/wiki/` directory is retired)
- A directive that **`docs/wbs/` is retired** in favor of in-conversation `TaskCreate`

This project (Kotoha) is the 5th of 12 to adopt the new structure (Phase C-5). Two project-specific challenges arise:

1. **WBS legacy**: `docs/wbs/` contains 35 historical implementation logs accumulated across Phase 0 through Phase 3-B. These are valuable as audit trails for the OSS-bound codebase and link to merged PRs by ISSUE number. Mass deletion would eliminate that history from the in-tree narrative even though git log retains the raw record.
2. **Glossary scale**: `docs/wiki/glossary.md` holds **101 terms** organized into 9 thematic sections. Splitting into 101 individual concept files in one PR while also rewriting each spec body is impractical.

The Kotoha project additionally departs from the global Japanese-only documentation rule: commit messages, PR titles/bodies, and GitHub ISSUEs are written in English to ease future OSS contribution flow (per the existing project-level `CLAUDE.md` exception).

## Decision

This PR performs only **mechanical processing** for the Phase C-5 migration. Three project-specific exceptions are recorded here.

### A. Bulk frontmatter and rename

- Move `docs/superpowers/{specs,plans}` to `docs/{specs,plans}` via `git mv`
- Rename every spec to `_uncategorized/<feature-slug>.md` form (drop date prefix, kebab-case slug)
- Inject the 7-field global frontmatter, replacing the existing project-local frontmatter (`title`/`date`/`status`/`phase`/`revision` keys)
- Status is inferred from ROADMAP completion state and ISSUE state; `related_issues` is taken from ROADMAP cross-references
- 14-section spec body restructure is **deferred** to feature PRs (the same line as `MITRA_X#1212` ADR 0001 and `infrastructure#30` ADR 0002)

### B. WBS retention exception

The 35 entries under `docs/wbs/` are kept as historical artifacts of the project. The global retirement of `docs/wbs/` is **partially adopted**:

- **New WBS entries are forbidden.** All future per-task tracking uses in-conversation `TaskCreate` (per global rule).
- Existing entries remain as reference material. They are not migrated to any other location (they are implementation logs, not specs).
- The project `CLAUDE.md` `WBS 直接 push の例外` clause is preserved for **edits to existing WBS files only** (typo fixes and post-merge log updates), not for new file creation.

The exception expires when the legacy WBS material is no longer being referenced from active spec / plan / commit message bodies. A cleanup PR may then move `docs/wbs/` to `docs/_archive/wbs/` or remove it entirely.

### C. Glossary partial migration (canonical relocation, no content rewrite)

The 101 terms in `docs/wiki/glossary.md` are migrated to `~/Obsidian/kotoha-ime/glossary/<concept>.md` as one file per concept. The migration is **mechanical**:

- The 9-section taxonomy of the source becomes nine `#cluster/<name>` tags
- Each concept's body is preserved verbatim from the source
- `aliases` are extracted from heading parentheses and explicit `**別名**` markers
- `related_concepts` is **left empty in this PR**; cross-linking is deferred to future Obsidian graph-view curation

The original `docs/wiki/glossary.md` file is removed after migration. The `docs/wiki/` directory is removed.

## Consequences

- After merge, 8 specs hold only frontmatter; their bodies are unchanged
- 101 concept files exist under `~/Obsidian/kotoha-ime/glossary/`
- 35 WBS files remain under `docs/wbs/` and are subject to the project-level exception clause in `CLAUDE.md`
- Future feature PRs for Phase 3-B residuals (B0h-f / B3 / B6), Phase 5, and beyond will be expected to:
  - Restructure the relevant spec body to the 14-section global format (one spec at a time)
  - Cross-link `glossary_refs` in spec frontmatter to vault concept files
  - Avoid creating new WBS entries (use in-conversation `TaskCreate` instead)

## References

- Pilot ADR lineage: `MitraDataScience/MITRA_X#1212` ADR 0001 (bulk frontmatter); `koh-hinooka/infrastructure#30` ADR 0002 (bulk frontmatter + cloud-sql exit strategy + retroactive release ADR waiver)
- Global rules: `~/.claude/CLAUDE.md` §Documentation Structure / §Spec Frontmatter / §Spec Clustering / §Persistent Memory ("What Goes Where")
- Project rules: `CLAUDE.md` `Language 例外` (English doc convention) and `WBS 直接 push の例外` (now narrowed to existing-file edits only)
