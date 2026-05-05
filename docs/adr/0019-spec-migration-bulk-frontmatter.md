# 0019: spec migration bulk frontmatter strategy and project-specific exceptions

## Status

Accepted (2026-05-05) — Amended 2026-05-05: §B repealed (PR #180)

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

### B. WBS retention exception (REPEALED 2026-05-05)

> **Status**: REPEALED in PR #180 (2026-05-05). All 36 WBS entries migrated to `docs/specs/_uncategorized/` as `status: implemented` specs. The `docs/wbs/` directory has been removed. Project now fully aligned with global `~/.claude/CLAUDE.md` §Documentation Structure.

#### Original decision (historical record, no longer in effect)

The 35 entries under `docs/wbs/` were originally kept as historical artifacts with new-entry ban + machine-enforced exit conditions deferred to 2026-12-31. See git history for full original wording.

#### Repeal rationale

User direction (2026-05-05): "Global CLAUDE.md 規約を優先" — the partial-adoption exception was lifted in favor of full alignment with the global rule. WBS files were treated as eligible for spec migration (despite being implementation logs by nature), accepting the looser interpretation of "spec" to encompass post-hoc implementation records with `status: implemented`.

#### Migration outcome (PR #180)

- 36 WBS files (35 actual + `template.md` excluded) migrated to `docs/specs/_uncategorized/<slug>.md`
- spec slug derived from WBS filename minus leading `<yyyy-MM-dd>-` prefix
- `feature: <slug>`, `status: implemented`, `bounded_context: _uncategorized`, `related_issues: ["#NNN"]` (from filename), `last_reviewed: 2026-05-05`
- Original WBS body preserved verbatim under `## 元 WBS 内容 (実装ログ由来)` section
- 14-section restructure deferred to per-spec follow-up (project-deep curation work)
- `docs/wbs/` directory deleted entirely
- Project `CLAUDE.md` `WBS 直接 push の例外` clause removed
- Mechanical enforcement of "no new WBS entries" now redundant (directory does not exist; would have to be re-created intentionally)

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
- ~~35 WBS files remain under `docs/wbs/` and are subject to the project-level exception clause in `CLAUDE.md`~~ → REPEALED 2026-05-05: 36 WBS migrated to `docs/specs/_uncategorized/` as `status: implemented`, `docs/wbs/` directory deleted, `CLAUDE.md` exception clause removed (PR #180). Project fully aligned with global rule.
- Future feature PRs for Phase 3-B residuals (B0h-f / B3 / B6), Phase 5, and beyond will be expected to:
  - Restructure the relevant spec body to the 14-section global format (one spec at a time)
  - Cross-link `glossary_refs` in spec frontmatter to vault concept files
  - Use in-conversation `TaskCreate` for per-task tracking (global rule, no project exception)

### D. Retroactive SemVer mapping waiver

This PR back-fills SemVer milestones into `docs/ROADMAP.md` "完了済" section: `v0.0.0` (Phase 0 Foundation), `v0.1.0` (Phase 1 Kana→Kanji conversion), and `v0.2.0` (Phase 2 Dictionary and learning). The global `~/.claude/CLAUDE.md` §Milestone Specification completion criteria require, for each milestone, an annotated git tag (`vX.Y.Z`) AND a release ADR (`docs/adr/NNNN-release-vX.Y.Z.md`).

For these three back-filled milestones, the requirements are **waived** with the following rationale:

1. The original tag-time HEAD cannot be reconstructed reliably (the SemVer label was not in use when the milestones merged; choosing a commit retroactively would introduce a release marker that diverges from the actual deploy boundary)
2. A retroactive release ADR would describe historical decisions reconstructed after the fact and would not benefit the current codebase
3. The follow-up enforcement value (audit trail, traceability) is satisfied by ROADMAP entries and the existing per-Phase ADRs (0001〜0018)

The §Milestone Specification completion criteria are **strictly enforced from `v0.3.0` onward** (the active milestone in this PR). This waiver follows the precedent of `koh-hinooka/infrastructure#30` ADR 0002 §"v1.0.0 / v1.1.0 リリース ADR の遡及不要決定" with the same three-point rationale.

## References

- Pilot ADR lineage: `MitraDataScience/MITRA_X#1212` ADR 0001 (bulk frontmatter); `koh-hinooka/infrastructure#30` ADR 0002 (bulk frontmatter + cloud-sql exit strategy + retroactive release ADR waiver)
- Global rules: `~/.claude/CLAUDE.md` §Documentation Structure / §Spec Frontmatter / §Spec Clustering / §Persistent Memory ("What Goes Where") / §Milestone Specification
- Project rules: `CLAUDE.md` `Language 例外` (English doc convention) and `WBS 直接 push の例外` (now narrowed to existing-file edits only)
