# ADR 0011 — Kanji backend trait design (Backend trait + BackendConfig non_exhaustive enum)

- **Status**: Accepted (2026-04-25)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 1 maintainers

## Context

Kotoha Phase 1 (Kana→Kanji conversion) では、複数の推論 backend を同一の API 形で取り扱う抽象が必要である。Phase 1 の milestone P1-1 では決定的な test 用 `MockBackend` を skeleton として用意し、P1-2 で llama.cpp 経由の具象実装 `ZenzBackend` を追加した後、P1-2.5 refactor (PR #74 merge `02cf035`) で当該 backend を `LlamaCppBackend` に汎用化した。Phase 5 (ADR 0010) では新たな `KotohaNative` backend (Kotoha 専用 romaji-base モデル + HuggingFace tokenizer 直接 load) の追加が計画されており、2 系以上の backend を構造的に dispatch する抽象を Phase 1 のうちに確立しておく必要があった。

Rust で複数実装を単一 API で受け取る慣用手段は、(a) `trait` + `Box<dyn Trait>`、(b) `trait` + enum dispatch (variant ごとに concrete struct を wrap する enum)、(c) build-time feature の排他切替による concrete struct 直 use の 3 種である。本 ADR は Phase 1 の実装で採用した (a) への決定理由を記録する。

関連する Phase 1 実装ファイルは以下である。

- `crates/kotoha-core/src/kanji/backend.rs` — `KanjiBackend` trait、`BackendConfig` enum、`PromptTemplate` enum、`load_backend` factory、共有 helper (`validate_input` / `score_sort_dedupe`) を定義する
- `crates/kotoha-core/src/kanji/mock.rs` — `MockBackend` (feature = `mock-backend`)
- `crates/kotoha-core/src/kanji/llama_cpp.rs` — `LlamaCppBackend` (feature = `llama-cpp`)

## Decision

本 ADR では以下 5 項目を決定する。

### D1. `KanjiBackend` trait を `crates/kotoha-core/src/kanji/backend.rs` に定義する

Method は以下 2 本とする (実装は PR #74 merge `02cf035` で final 形に収束している)。

```rust
pub trait KanjiBackend {
    fn model_id(&self) -> &str;

    fn convert(
        &self,
        input: &str,
        options: &ConvertOptions,
    ) -> Result<Vec<Candidate>, KanjiError>;
}
```

- `convert` は hiragana 列 + `ConvertOptions` を受け取り `Vec<Candidate>` を返す
- `model_id` は debug / log 用の文字列を返す (例: `"mock"` / `"gemma-2-2b-jpn-it-Q5_K_M"`)
- 事前条件 / 事後条件 / invariant は Design by Contract で rustdoc に記述する (spec §5.3 / §5.6 / §5.7 と相互参照、実装済)
- Phase 1 は single-thread CLI のみのため `Send + Sync` 境界は要求しない。Phase 3 IBus 統合時に必要性を再評価する

### D2. `BackendConfig` enum を `#[non_exhaustive]` で定義する

```rust
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum BackendConfig {
    Mock,
    LlamaCpp {
        model_path: PathBuf,
        prompt_template: PromptTemplate,
    },
}
```

`#[non_exhaustive]` 属性は ADR 0006 (non-exhaustive on streaming enums) の方針と整合する。Phase 2 以降に `KotohaNative` / `Dictionary` / `RemoteHttp` 等の新 variant を追加する変更を、downstream crate にとって non-breaking に保つ。

`PromptTemplate` 側も同様に `#[non_exhaustive]` とし、Phase 2+ で `Phi4InstructChat` / `Llama3Chat` 等の family tag を追加可能にしている。

### D3. `load_backend(config: &BackendConfig) -> Result<Box<dyn KanjiBackend>, KanjiError>` factory で dispatch する

Config から backend を返す factory を 1 本用意し、backend 構築ロジックを呼出し側から分離する。

```rust
pub fn load_backend(
    config: &BackendConfig,
) -> Result<Box<dyn KanjiBackend>, KanjiError> {
    match config {
        #[cfg(feature = "mock-backend")]
        BackendConfig::Mock => Ok(Box::new(crate::kanji::MockBackend::new())),
        #[cfg(not(feature = "mock-backend"))]
        BackendConfig::Mock => Err(KanjiError::FeatureDisabled {
            feature: "mock-backend",
        }),
        #[cfg(feature = "llama-cpp")]
        BackendConfig::LlamaCpp { model_path, prompt_template } => {
            Ok(Box::new(crate::kanji::LlamaCppBackend::load(
                model_path,
                prompt_template.clone(),
            )?))
        }
        #[cfg(not(feature = "llama-cpp"))]
        BackendConfig::LlamaCpp { .. } => Err(KanjiError::FeatureDisabled {
            feature: "llama-cpp",
        }),
    }
}
```

Feature 未有効時は `KanjiError::FeatureDisabled { feature: &'static str }` で明示的に失敗させ、build の軽量化と実行時の診断容易性を両立する。

### D4. `dyn Backend` を選び enum dispatch は採用しない

`BackendConfig` は enum だが、factory の返り値は `Box<dyn KanjiBackend>` の trait object である。Phase 5 以降に `crate::kanji` 外から新 backend を追加する可能性を残すため、trait を外部実装可能な形で維持する。enum dispatch (`BackendImpl::Mock(MockBackend) | BackendImpl::LlamaCpp(LlamaCppBackend)`) を採用すると、外部 crate が独自 backend 実装を追加する際に enum variant も増やす breaking change が必要となる。

`dyn Backend` の動的 dispatch overhead は、`convert` 1 回あたりの LLM inference (Phase 1 default で約 3 秒 warm / 約 10 秒 cold) に比べて無視可能な桁である。

### D5. 共有 helper `validate_input` / `score_sort_dedupe` を `pub(crate)` で backend.rs に配置する

入力契約検査 (hiragana-only / 128 char 上限) と出力整形 (score 降順 sort + surface dedupe + top_k truncate) を trait 外の helper として共有し、`MockBackend` / `LlamaCppBackend` の双方が同一 logic を呼ぶ構造にする。これにより trait 実装者が契約逸脱の logic を書くリスクを削減する。helper の単体テストは backend.rs 末尾の `#[cfg(test)] mod tests` で網羅する (実装済)。

## Consequences

### 正の帰結

- Phase 5 の `KotohaNative` backend 追加が、(a) `BackendConfig` の新 variant、(b) 新 struct 型の `KanjiBackend` 実装、(c) `load_backend` の match arm 追加、の 3 点のみで完了する。既存 variant や既存実装の変更を伴わない
- `#[non_exhaustive]` により `BackendConfig` および `PromptTemplate` の新 variant 追加は downstream にとって non-breaking change として扱える
- test では `MockBackend` を、本番では `LlamaCppBackend` を同一 API で扱えるため、integration test が実 model 無しで実行可能となった (Phase 1 lefthook pre-push が default features のみで完結する)
- `validate_input` / `score_sort_dedupe` の共有により、契約逸脱の実装 bug が `MockBackend` / `LlamaCppBackend` の一方にのみ混入する risk が排除された

### 負の帰結

- `Box<dyn KanjiBackend>` の仮想 call overhead が存在する。ただし Phase 1 の `convert` 呼出しは 1 回 = LLM inference (秒単位) のため、実効的な影響は無視可能である
- `load_backend` の返り値 `Result<Box<dyn KanjiBackend>, KanjiError>` は backend-specific な詳細 error を `KanjiError::Backend { reason: String }` に文字列化して集約する構造のため、呼出し側から backend 固有の error 種別を match で特定することはできない (Phase 2 で CLI / IBus engine 着手時に `Backend { kind: BackendErrorKind, source: ... }` 型への拡張を検討する、WBS `2026-04-24-feature-73-...md` の follow-up 2 参照)
- trait の `Send + Sync` 境界を課していないため、Phase 3 以降に IBus engine 層がマルチスレッド化する場合、backend 実装側の内部状態に対する追加の境界検討が必要となる

## Alternatives considered

以下 3 案を検討し、いずれも棄却した。

### A. concrete struct 1 本 + `#[cfg(feature)]` で backend を切替える

**Rejected.** `MockBackend` と `LlamaCppBackend` が build 時に排他となり、integration test で mock 経由の pipeline を検証する際に real backend が build から外れる。lefthook pre-push (default features) と Layer 3 smoke (`llama-cpp-smoke` feature) が並立する Phase 1 の test 戦略が成立しない。

### B. enum dispatch (`BackendImpl::Mock(Mock) | BackendImpl::LlamaCpp(LlamaCpp)`)

**Rejected.** `crate::kanji` 外から独自 backend 実装を追加する経路を塞いでしまう。Phase 5 は `KotohaNative` backend を Phase 1 コードを改変せず追加する設計方針であり、enum variant 増加を強いる dispatch 形は Phase 5 拡張性と両立しない。Phase 1 scope では dispatch overhead の削減より拡張性を優先する。

### C. trait object + enum を設けず factory が文字列 / 構造体を直接 match する

**Rejected.** config の parse ロジックと backend 構築ロジックが factory 1 本に混入し、config 型の単体テストが factory 経由でしか書けなくなる。`BackendConfig` を明示的な enum として分離することで、config の型 assertion と backend 生成を独立に test 可能にした (Phase 1 実装では `backend_config_is_clone` / `backend_config_debug_contains_variant_name` 等の config 型 unit test が成立している)。

## Related documents

- 実装: `crates/kotoha-core/src/kanji/backend.rs`
- `MockBackend` 実装: `crates/kotoha-core/src/kanji/mock.rs`
- `LlamaCppBackend` 実装: `crates/kotoha-core/src/kanji/llama_cpp.rs`
- Phase 1 spec §5.3 / §5.4 / §5.6 / §5.7: `docs/superpowers/specs/2026-04-24-kotoha-phase-1-design.md`
- Phase 5 拡張計画 (`KotohaNative` backend): ADR 0010 (`docs/adr/0010-kotoha-custom-romaji-base-model.md`) D6
- `#[non_exhaustive]` 方針: ADR 0006 (`docs/adr/0006-non-exhaustive-on-streaming-enums.md`)
- feature flag 分離方針: ADR 0012 (`docs/adr/0012-feature-flag-design-for-llama-cpp.md`)
- Phase 1 default model 採用理由: ADR 0009 (`docs/adr/0009-phase-1-default-model-selection.md`)
- PR #74 (P1-2.5 refactor で trait が最終形に収束): merge commit `02cf035`
