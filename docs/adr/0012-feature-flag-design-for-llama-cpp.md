# ADR 0012 — Feature flag design for llama.cpp integration

- **Status**: Accepted (2026-04-25)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 1 maintainers

## Context

Kotoha Phase 1 の `LlamaCppBackend` は `llama-cpp-2` crate (utilityai 配下、Phase 1 では 0.1.145 pin) に依存し、`llama-cpp-2` は llama.cpp (C++) を bindgen 経由で FFI する。すなわち Kotoha workspace を build する際、`llama-cpp-2` を有効にする設定では C++ toolchain (CMake + C++ compiler) が必須となり、cold build は Kotoha 全体で数分〜10 分規模になる (リンク時間を含む)。

本プロジェクトは lefthook pre-push で `cargo build --workspace` + `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` を実行する方針 (project CLAUDE.md の Sub-agent Self-Report is Untrusted セクション参照) を採用しているため、pre-push で毎回 llama-cpp-2 を compile すると push cycle が秒単位から分単位に劣化し、開発速度が大きく低下する。

他方、llama-cpp 周りのコード (`LlamaCppBackend` + prompt 組立 + tokenization + inference loop) を常時 compile 対象から外してしまうと、当該コードの型検査が CI で走らず、refactor 時の regression を検出できなくなる。

加えて Layer 3 smoke test (`crates/kotoha-core/tests/kanji_llama_cpp_smoke.rs`) は実際の GGUF model (Phase 1 default: Gemma-2-2B-jpn-it Q5_K_M、約 1.92 GB) を load するため、model 未配置環境 (CI 常駐 runner / contributor の初期 clone 直後 / CI 軽量マシン等) で blind に実行するとテストが load error で fail する。そのため smoke は opt-in で、かつ model 未配置環境でも SKIP として exit 0 で終わる必要がある。

本 ADR はこれらを全て満たす feature flag 設計を記録する。Phase 1 実装は PR #74 (feature 命名 `zenz` → `llama-cpp` rename) と PR #82 (CLI 側の feature propagation) で完了している。

## Decision

本 ADR では以下 5 項目を決定する。

### D1. `kotoha-core` crate に 2 feature を置く: `llama-cpp` と `llama-cpp-smoke`

`crates/kotoha-core/Cargo.toml` に以下を置く (実装済)。

```toml
[features]
default = []
mock-backend = []
llama-cpp = ["dep:llama-cpp-2"]
llama-cpp-smoke = ["llama-cpp"]
```

- `llama-cpp`: `llama-cpp-2` を optional dependency から実体 dependency に昇格し、`LlamaCppBackend` の compile を有効化する
- `llama-cpp-smoke`: 上記 `llama-cpp` を implies した上で、Layer 3 smoke test (`tests/kanji_llama_cpp_smoke.rs`) を cfg gate で有効化する
- `mock-backend`: 本 ADR の scope 外だが、`default` に含めない原則 (後述 D5) と同じ理由で feature 化している

### D2. `kotoha-cli` crate に 1 feature を置き、`kotoha-core` に propagate する

`crates/kotoha-cli/Cargo.toml` に以下を置く (実装済)。

```toml
[features]
default = []
llama-cpp = ["kotoha-core/llama-cpp"]
```

CLI 側の `llama-cpp` feature は `kotoha-core/llama-cpp` を活性化するのみで、CLI 自身は FFI 固有コードを持たない。この propagation により、CLI 利用者は `cargo build -p kotoha-cli --features llama-cpp` という 1 指定で backend を含めた完全 build ができる。

### D3. `[[bin]] kotoha-kanji` に `required-features = ["llama-cpp"]` を指定する

`kotoha-cli/Cargo.toml` の `[[bin]] kotoha-kanji` に `required-features` を付与することで、default features での `cargo build --workspace` は当該 binary を skip する。これにより以下が成立する。

- default build が C++ toolchain 不要で完結する
- 誤って `cargo build --workspace` で `kotoha-kanji` binary が link error を出す事象が発生しない
- `kotoha-kanji` を build する意図は `--features llama-cpp` の明示で表明される

### D4. Layer 3 smoke test は `#[cfg(feature = "llama-cpp-smoke")]` で gate し、model 未配置時に SKIPPED とする

`tests/kanji_llama_cpp_smoke.rs` は module-level で `#[cfg(feature = "llama-cpp-smoke")]` を持ち、feature 未有効時は test 関数が 1 個も compile されない。feature 有効時は `KOTOHA_LLAMA_MODEL_PATH` 環境変数を読み、未設定の場合は `eprintln!("SKIP: ...")` + `return;` で各 test を skip 扱いにする (cargo test exit 0)。

この設計により以下が成立する。

- lefthook pre-push (default features) では smoke test が compile されず、C++ toolchain + GGUF model path 不要で push が通る
- `cargo test --features llama-cpp-smoke` を明示した contributor 機で model 未配置なら SKIP、配置済みなら実 inference を実行する

### D5. `default = []` を厳守する

`cargo build --workspace` / `cargo test --workspace` (feature 指定無し) は C++ toolchain 不要で PASS する。新 contributor が `git clone` 直後に `cargo build --workspace` を走らせた時、初期化に llama.cpp C++ build を要求しない。これにより Phase 1 の onboarding 体験と lefthook pre-push 速度を両立する。

## Consequences

### 正の帰結

- lefthook pre-push が高速に完了する (llama-cpp-2 の cold build を毎回走らせない)
- model 未配置環境 (contributor の clone 直後、CI 軽量 runner) でも default build 成功
- smoke は opt-in で `--features llama-cpp-smoke` を明示して実行する (意図が明確)
- `required-features` により `kotoha-kanji` binary は「llama-cpp を要求する」ことが Cargo.toml で静的に明示される
- `kotoha-core` / `kotoha-cli` の 2 crate で feature 名が揃っている (両方とも `llama-cpp`) ため、利用者の mental model が簡素である

### 負の帰結

- `cargo clippy --all-features` を走らせると `llama-cpp-2` が build 対象に入り、clippy 時間が大きく延びる (数分規模)。日常の clippy は feature 指定無しで走らせ、`--all-features` 版は release 前の手動検証として位置付けている
- feature の組合わせが 3 通り (`default` / `llama-cpp` / `llama-cpp-smoke`) あり、`Cargo.toml` の [features] 記述がやや冗長である。ただし組合わせ数の増加は Phase 5 でも 4 通り程度に留まる見込みで、単純 enum 的な管理で維持可能である
- `llama-cpp-smoke` の model path 欠落 SKIP は test 関数内部で早期 return するため、「smoke テストが『走ったが何も assert しなかった』状態」を偽装する可能性がある。本 ADR では CLI / README / WBS で「smoke 実行は必ず `KOTOHA_LLAMA_MODEL_PATH` 設定を伴う」という運用規則を明示的に記録して補う

## Alternatives considered

以下 3 案を検討し、いずれも棄却した。

### A. 単一 feature `llama` で binding + smoke を同時に on にする

**Rejected.** binding を on にして smoke を off にする組合せが表現不可能となり、「C++ build のみ検証したい (smoke 無し)」use case を塞ぐ。また lefthook pre-push で「binding のみの compile 検査」を回したい場合に smoke が同時に on になって fail する。2 feature 分離が構造的に不可欠である。

### B. binary を別 crate (`kotoha-kanji`) に分離する

**Rejected.** workspace に crate を 1 本増やすことで Phase 1 完了時点の crate 数が 3 に増える。Phase 2 以降に `kotoha-dict` / `kotoha-ibus` / `kotoha-fcitx5` 等の crate 追加が予想されるため、現時点で binary 分離を強行する必然性は弱い。`required-features` による仮想的な分離で Phase 1 の要求は満たされる。

### C. always-on llama-cpp (feature なしで常時 build)

**Rejected.** default build が C++ toolchain 必須になり、onboarding 体験と CI 高速化目標の両方と矛盾する。Kotoha は今後 Phase 3 (IBus) / Phase 4 (fcitx5) / Phase 5 (Kotoha custom model) で binding 追加が見込まれるため、feature 化の orchestration は Phase 1 のうちに固めておく必要がある。

## Related documents

- `crates/kotoha-core/Cargo.toml` — `default` / `mock-backend` / `llama-cpp` / `llama-cpp-smoke` 4 feature 定義
- `crates/kotoha-cli/Cargo.toml` — `default` / `llama-cpp` 2 feature 定義 + `[[bin]] kotoha-kanji` の `required-features`
- Phase 1 spec §4.3 / §8.3: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- backend 抽象設計 (`KanjiBackend` trait と `BackendConfig` enum): ADR 0011
- Phase 1 default model: ADR 0009
- PR #74 (feature 命名 `zenz` → `llama-cpp` rename、merge `02cf035`)
- PR #82 (CLI feature propagation + `kotoha-kanji` binary、merge `f9a820c`)
