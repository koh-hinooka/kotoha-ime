# ADR 0009 (prep note) — Kanji backend / model selection pivot to Gemma-2-2B-jpn-it

- **Status**: Prep note (superseded by a full ADR authored in P1-4)
- **Date**: 2026-04-24
- **Deciders**: Kotoha Phase 1 maintainers
- **Context**:
  - Spec §3.2 originally designated `Zenz-v2.5-medium` as the Phase 1 default model.
  - P1-2-9 empirical verification (WBS 718fd8e) surfaced that every Miwa-Keita Zenz GGUF (`v1 / v2 / v2.5-medium / v3.1-small / v3.1-xsmall`) relies on `tokenizer.ggml.pre = "gpt2-small-japanese-char"`, which is absent from the llama-cpp-2 0.1.145 pre-tokenizer allow-list (and from upstream llama.cpp master).
  - Version bumping llama-cpp-2 within 0.1.x does not resolve the blocker — the upstream llama.cpp does not yet ship support.
  - A 3-way empirical comparison (Qwen2.5-1.5B-Instruct Q5_K_M / Gemma-2-2B-jpn-it Q5_K_M / Gemma-3-1B-it Q5_K_M) selected Gemma-2-2B-jpn-it as the only model passing 5/5 cases including honorific handling.
  - P1-2.5-8 empirical verification against the final refactored `LlamaCppBackend` (via llama-cpp-2 0.1.145 `apply_chat_template`) required an IME-style multi-turn few-shot prompt wrapper (`build_chat_tuples`); the bare `apply_chat_template(None)` path alone did not make Gemma-2-2B-jpn-it perform kana→kanji conversion (it returned conversational echo + emoji noise). Final fixture size: 9 rows, 9/9 PASS.

- **Decision (to be formalized in P1-4)**:
  1. Adopt **Gemma-2-2B-jpn-it Q5_K_M** (Gemma License, 1.92 GB) as the Phase 1 default model.
  2. Rename `ZenzBackend` → `LlamaCppBackend` to make the backend llama.cpp-family agnostic.
  3. Route prompt construction through `llama-cpp-2 apply_chat_template`, dispatched by `PromptTemplate` (`Gemma2InstructChat` / `Qwen2Chat` / `Custom`). The Gemma2InstructChat / Qwen2Chat variants read the GGUF-embedded `tokenizer.chat_template` via `LlamaModel::chat_template(None)`.
  4. Embed an IME-style multi-turn few-shot prompt wrapper in `build_chat_tuples` for Gemma2InstructChat / Qwen2Chat (3 pre-filled user/assistant examples + actual query), because chat-tuned Gemma-2-2B-jpn-it requires an instruction demonstration to perform conversion rather than conversational response.
  5. Remove hiragana→katakana preprocessing from the kanji subsystem; spec §5.6 keeps hiragana-only at the API boundary only.
  6. Retain the Zenz family as a Phase 2 re-evaluation target contingent on upstream llama.cpp adding `gpt2-small-japanese-char` to the pre-tokenizer allow-list.

- **Consequences**:
  - +: Phase 1 acceptance gate (smoke 5+ cases) is unblocked — current fixture passes 9/9.
  - +: `PromptTemplate` abstraction opens the door to Qwen 2 and future Phi-4 / Llama-3 variants without structural changes.
  - -: Gemma License is less permissive than Apache 2.0; Kotoha model distribution remains "user downloads from HuggingFace" for Phase 1.
  - -: Bundled GGUF chat template is a compliance hazard for private-fork quantizations that strip metadata; Phase 2+ may need a `Custom` template escape hatch.
  - -: Cold-load + 9-case inference (~30 s) is at the boundary of spec §8.3's 30-second latency target. Target relaxation or revision is deferred to P1-4.
  - -: `build_chat_tuples` embeds 3 hard-coded Japanese few-shot examples; the `Custom` variant is the escape hatch for integrators who need a different instruction style (e.g., conversational IME, English-input variants).

- **Alternatives considered**:
  - **A**: Wait for upstream llama.cpp to allow-list `gpt2-small-japanese-char`. Rejected because the Phase 1 schedule cannot accept an open-ended upstream dependency.
  - **B**: Implement the `gpt2-small-japanese-char` pre-tokenizer in a forked llama-cpp-2. Rejected because the maintenance cost (llama.cpp bindgen surface churn) outweighs the benefit at Phase 1 scale.
  - **C**: Ship a `candle-core` pure-Rust backend for Zenz. Rejected because Candle's GGUF tokenizer story for character-level + byte-level BPE is immature as of 2026-04.
  - **D**: Use raw hiragana input without instruction wrapper. Rejected because P1-2.5-8 empirical run yielded 1/15 pass rate (the model echoed input + emoji instead of converting).

- **Related documents**:
  - Implementation plan: `docs/superpowers/plans/2026-04-24-kotoha-phase-1-p1-2-5.md`
  - Empirical verification: `docs/wbs/2026-04-24-feature-69-zenz-backend-layer3-smoke.md` (commit `718fd8e`)
  - Spec revisions: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md` §3.2 / §3.3 / §5.4 / §5.6 / §6 / §10
  - Superseded by: (future ADR 0009 authored in milestone P1-4)
