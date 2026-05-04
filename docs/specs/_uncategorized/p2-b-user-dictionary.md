---
feature: p2-b-user-dictionary
status: implemented
bounded_context: _uncategorized
related_issues: []
related_prs: []
glossary_refs: ["user-dictionary", "kotoha-storage"]
last_reviewed: 2026-05-05
---

# Kotoha Phase 2 P2-B (User dictionary) 設計書

本設計書は Phase 2「Dictionary and learning」の 2 番目の milestone P2-B を詳細化する子 spec である。Phase 2 spec(parent-spec)が示す Phase 2 全体スコープの中で、P2-B は P2-A で先出しした `VocabularyLookup` trait の 2 つ目の実装(`CustomVocab` に続く)として `UserVocab` を新設し、ユーザ独自語彙の永続化 + lookup + CLI 編集機構を確立する。Phase 2 spec / ADR 0014 と本設計書の記述が矛盾する箇所は、本設計書が新しい(P2-B kick-off brainstorming で確定した最新方針)とする。

## 目次

- [1. 目的と範囲](#1-目的と範囲)
- [2. Out of scope](#2-out-of-scope)
- [3. 設計判断(brainstorming で確定した 11 項目)](#3-設計判断brainstorming-で確定した-11-項目)
- [4. アーキテクチャ詳細](#4-アーキテクチャ詳細)
- [5. Schema(SQLite v001)](#5-schemasqlite-v001)
- [6. Trait API と型](#6-trait-api-と型)
- [7. CLI 仕様](#7-cli-仕様)
- [8. DictionaryBackend 統合](#8-dictionarybackend-統合)
- [9. Validation](#9-validation)
- [10. Test 戦略](#10-test-戦略)
- [11. Effort estimate と PR 分割](#11-effort-estimate-と-pr-分割)
- [12. 参照](#12-参照)

## 1. 目的と範囲

### 1.1 P2-B の目的

P2-B は Kotoha Phase 2「Dictionary and learning」の 2 番目の milestone であり、Kotoha は P2-B でユーザ独自語彙(UserVocab)の永続化・lookup・CLI 編集機構を確立する。Kotoha は本 milestone において、P2-A で先出しした `VocabularyLookup` trait の 2 つ目の実装として `UserVocab` を新設し、SQLite ベースの永続化層 `kotoha-storage` crate を新規導入する。

P2-B の到達目標は以下 4 点である。

- Phase 2 G3「User 個別語彙を User dict への明示登録で覆える」を `kotoha-dict {add, remove, list, show}` の 4 subcommand で達成する
- P2-A 確定済の baseline(default 175 PASS / dict 223 PASS)を退行させない
- SQLite 共用 DB(`kotoha.db`)を Phase 2 全 milestone(P2-B / P2-C / P2-D)が再利用可能な形で先出しする
- ADR 0014 D7 改訂(`kotoha-storage` crate 導入 + SQLite 共用 DB 採用)と新規 ADR 0015(`kotoha-storage` SQLite 採用根拠)を本 P2-B docs PR で正式化する

### 1.2 親 spec / ADR / ROADMAP との関係

本 spec は parent-spec(Phase 2 spec)§3.2 Learning cache / §5.2 persistence / §6.1 既存 Backend trait 拡張 / §8 GUI dict editor / §9 Open questions の 5 節を詳細化する子 spec として位置づく。Phase 2 spec / ADR 0014 と矛盾する記述は、P2-B kick-off brainstorming(2026-04-25)で empirical に確定した最新方針として本設計書が優先する。具体的には、parent-spec §5.2 で「TSV 第一候補」と stub 化していた Learning cache 永続化 format は、本設計書 §3.1 / §3.4 / §3.5 で「SQLite 共用 DB(`kotoha.db`)を採用、format 比較は ADR 0015 で archived」と確定する。

ADR 0014 については、本 P2-B docs PR で D7 節を新規追加し「P2-B で `kotoha-storage` crate を導入し SQLite 共用 DB を採用する」を明文化する(§3.1 / §3.2 と整合)。新規 ADR 0015(`kotoha-storage` SQLite 採用)は本 P2-B docs PR の同一 commit で起票する。

### 1.3 ROADMAP §P2-B との関係

ROADMAP §P2-B(工数目安 3〜5 day)に対し、本 spec は実装範囲を確定する。範囲確定の結果、§11 で論じるとおり実工数は 6.5 day を見込み、ROADMAP 目安(3〜5 day)を超過する。超過理由(SQLite layer 厚み + validation 同時投入 + spec 同期更新)は §11.3 に記載し、3 PR 分割(P2-A hardening pre-PR + P2-B docs PR + P2-B code PR)で 1 PR あたりの規模を Branch Scope Policy 範囲内に収める。ROADMAP §P2-B は本 P2-B docs PR と同一 commit で更新する。

## 2. Out of scope

P2-B は以下 9 項目を扱わない。

- **LearningCache の insert / update / lookup / eviction 実装(P2-C)**: P2-B は `LearningCacheStore` trait skeleton と `learning_cache` table schema のみを先出しし、insert / update / lookup / eviction の実装本体は P2-C で行う
- **Hybrid backend / Ranker rerank(P2-D)**: P2-B は `DictionaryBackend` への UserVocab 組込のみを行い、LLM 候補 / Dictionary 候補 / LearningCache hit を merge / dedupe / rerank する Ranker 実装と `BackendConfig::Hybrid { llm, dict, learning }` 追加は P2-D で行う
- **`kotoha-dict` の `update` / `import` / `export` / `init` subcommand(Phase 6+)**: P2-B は `add` / `remove` / `list` / `show` の 4 subcommand のみを実装する。`update`(既存 entry の score / pos 変更)、`import`(TSV / CSV からの一括取込)、`export`(TSV / CSV への書出し)、`init`(空 DB の初期化)の 4 subcommand は Phase 6+ で扱う
- **`--reading` prefix match 以外の fuzzy / similarity / edit-distance 検索(Phase 5 / 6)**: P2-B の `kotoha-dict list --reading <PREFIX>` は SQL の prefix match(`reading LIKE 'prefix%'`)のみを提供し、編集距離・類似度・前方部分一致以外の fuzzy 検索は Phase 5 / 6 で扱う
- **GUI dict editor(Phase 6+ / 7)**: User dict を編集する GUI は Phase 6+ / 7 に送る。P2-B は CLI binary `kotoha-dict` のみを対象とする
- **schema migration の down / rollback(YAGNI defer)**: SQLite migration は forward-only とし、down migration / rollback は実装しない。recovery 経路は「DB 削除 + 再起動で空 DB 再作成」とする(§3.6 で根拠を記載)
- **`pos` 違いの同 (surface, reading) 多重保持**: UNIQUE 制約(surface, reading)で reject する。`pos` のみ異なる重複 entry の必要性が顕在化した場合は、別 ADR で UNIQUE 制約緩和を起票する(P2-B 時点では発生事例なし)
- **Phase 3 IBus DB lifecycle hook(Phase 3)**: IBus engine の GNOME Mutter spawn lifecycle に合わせた DB open / close / lock 戦略は Phase 3 で扱う。P2-B は `kotoha-dict` CLI 実行時のみ DB を open / close する
- **`kotoha-storage` の PostgreSQL feature flag(YAGNI、ADR 0015 で archived)**: PostgreSQL / Docker 採用案は ADR 0015 で却下した(§ADR 0015 Decision rationale)。`kotoha-storage` は SQLite 専用とし、別 RDBMS への切替 feature flag は持たない

## 3. 設計判断(brainstorming で確定した 11 項目)

P2-B kick-off brainstorming(2026-04-25)は以下 11 項目を empirical に確定した。各項目は本 P2-B 実装の根拠として機能し、後続 milestone(P2-C / P2-D)でも継承する。

### 3.1 Persistence: SQLite 採用(ADR 0015 起票根拠)

P2-B は UserVocab + LearningCache の永続化層に SQLite を採用する。

- **採用理由**: ACID + WAL によるアプリレベル lock 不要、`PRAGMA user_version` で正式 versioned migration、Phase 5 personalization で field 追加(context_embedding 等)が schema migration で完結
- **TSV / JSONL / TOML 棄却理由**: schema 進化 / concurrent 安全 / crash 安全 / escape edge case の 4 軸で SQLite が優位(詳細比較表は ADR 0015)
- **PostgreSQL 棄却理由**: Docker daemon 起動が IME runtime の前提条件になり distro 配布難度が爆発、container resource footprint(数百 MB)が許容圏外、Phase 3 IBus engine の GNOME Mutter spawn lifecycle と整合不能(詳細は ADR 0015)

### 3.2 Crate 配置: 新 crate `kotoha-storage` 導入(DIP)

P2-B は永続化層を新 crate `kotoha-storage` として独立させる。

- **採用理由**: Clean Architecture「Interface 依存」/ SOLID DIP に整合、`rusqlite` の C 依存(SQLite C library)を 1 crate に閉じ込めることで `kotoha-core` 単体 build は C コンパイル不要となる
- **kotoha-core 内配置棄却理由**: `kotoha-core` が `rusqlite` への直接依存を持つと、Layer 1 unit test / `MockBackend` 経路でも C library コンパイルを要求し、Phase 1 baseline の build 速度を退行させる
- **依存方向**: `kotoha-cli` → `kotoha-core` → `kotoha-storage`(`kotoha-storage` は `kotoha-core` に逆依存しない)

### 3.3 Romaji ↔ Hiragana: 内部は hiragana canonical、CLI 層 auto-detect

P2-B は DB 内部の `reading` field を hiragana canonical(`U+3040..=U+309F + U+30FC + U+30FB`)で統一する。CLI 層では `reading` 引数を auto-detect し、ASCII 入力時は `RomajiConverter` で hiragana に変換する。

- **採用理由**: P2-A `VocabularyLookup` trait の precondition(reading は hiragana)と整合、DB 検索時の正規化 cost を起動時 1 回で完結
- **CLI auto-detect 仕様**: 全 hiragana 入力 → そのまま、全 ASCII 入力 → `RomajiConverter` で変換、混在入力 → reject(exit code 2)
- **棄却した代替**: DB に romaji を保存して検索時に変換する案は、検索 cost が線形になり LearningCache lookup の latency budget を超過するため不採用

### 3.4 共用 DB: `kotoha.db` 単一、`user_vocab` + `learning_cache` table 同梱

P2-B は SQLite DB ファイルを `kotoha.db` 単一とし、UserVocab と LearningCache の table を同 DB 内に配置する。

- **採用理由**: ATOMIC transaction を user_vocab + learning_cache 跨ぎで取得可能、DB ファイル数を 1 個に維持することで運用 / backup / 削除が簡素化
- **別 DB 案棄却理由**: `user_vocab.db` + `learning_cache.db` の 2 ファイル分離案は、Phase 5 personalization で 2 table の cross-reference が必要になった場合に migration cost が発生
- **schema 影響**: `user_vocab` table と `learning_cache` table は外部キー制約を持たない(両 table の独立性を維持、§5 参照)

### 3.5 SQLite library: `rusqlite + bundled` 採用、async 系不採用

P2-B は SQLite library として `rusqlite`(`bundled` feature 有効)を採用する。

- **採用理由**: `rusqlite` は同期 API で `kotoha-core` の sync 設計と整合、`bundled` feature により SQLite C library を crate 同梱でビルド(distro 配布時の system SQLite version 差異を回避)
- **sqlx / tokio-rusqlite 棄却理由**: async 系 SQLite library は `kotoha-core::dict::user_vocab::UserVocab::lookup` の sync API と矛盾し、`tokio` runtime を `kotoha-core` に持ち込む追加コストを正当化できない
- **diesel 棄却理由**: ORM 層は P2-B の単純な CRUD 4 操作に対しオーバースペック、コンパイル時間と学習コストが正当化できない

### 3.6 Migration: `PRAGMA user_version` + `include_str!` 同梱 SQL、down は実装しない

P2-B は SQLite migration を `PRAGMA user_version` + `include_str!` で同梱した SQL ファイルで管理する。down migration は実装しない(YAGNI)。

- **採用理由**: 定数配列 `MIGRATIONS: &[(i32, &str)]` に `(version, sql_text)` を並べ、起動時に `user_version` を読み取り未適用 migration を順次 apply、SQL は `include_str!("migrations/v001_initial.sql")` でバイナリ同梱
- **forward-only 採用理由**: down migration は IME runtime の rollback 経路として使用しない(crash recovery は DB 削除 + 再作成で十分)、forward-only にすることで実装行数が約半分に縮小
- **migration tool 棄却理由**: `refinery` / `sqlx::migrate!` 等の外部 migration tool は依存追加コストが正当化できず、`PRAGMA user_version` の simplicity を優先

### 3.7 Lookup priority: `Vec<Box<dyn VocabularyLookup>>` の順序 `[CustomVocab, UserVocab]`、score tie 時 CustomVocab 勝ち残らせ

P2-B は `DictionaryBackend.vocabs: Vec<Box<dyn VocabularyLookup>>` の順序を `[CustomVocab, UserVocab]` で固定する。score tie 時は CustomVocab 由来の entry を勝ち残らせる。

- **採用理由**: `score_sort_dedupe` helper は同一 surface に対し score 降順で先勝ち選別を行うため、`Vec` の前方に CustomVocab を配置することで score 同値時に curated `kotoha-dict.tsv` の entry が user 個別 entry を上回る
- **業務的根拠**: CustomVocab は curation policy(`feedback_vocab_grammatical_collision.md` 参照)を経て追加された語彙であり、文法的多義性 check 済。UserVocab は user 自由入力であり curation がない。両者が同 score で衝突した場合、curation 済の CustomVocab を default で優先する
- **将来の override**: user が CustomVocab の特定 entry を上書きしたい場合、より高い score を `kotoha-dict add --score` で指定する(明示 opt-in)

### 3.8 CLI subcommand: `add` / `remove` / `list` / `show` の 4 本

P2-B は `kotoha-dict` binary に `add` / `remove` / `list` / `show` の 4 subcommand を実装する。`update` / `import` / `export` / `init` は将来送り(§2 Out of scope)。

- **採用理由**: P2-B の到達目標(G3「User 個別語彙を User dict への明示登録で覆える」)は `add` で entry 追加、`remove` で削除、`list` で確認、`show` で詳細表示の 4 操作で達成できる
- **`update` 棄却理由(P2-B 範囲外)**: `update` は既存 entry の score / pos 変更を扱うが、P2-B では「remove + add」で代替可能であり、独立 subcommand を持つ実装コストを後続 PR に送る
- **`init` 棄却理由(P2-B 範囲外)**: DB 初期化は `add` 実行時に `Database::open` 経由で lazy 実行されるため、explicit `init` subcommand は不要

### 3.9 Validation: hiragana-only reading + control char / bidi reject + finite non-negative score、#94 A03 を P2-B 着地時に同時投入

P2-B は validation を以下 4 軸で実施する。

- **`reading` field**: hiragana(U+3040..=U+309F) + 長音「ー」(U+30FC) + 中黒「・」(U+30FB) のみ許容、CLI normalize 後でも DB 層で再検証
- **`surface` field**: non-empty / ≤256 bytes / no control char(U+0000..=U+001F + U+007F) / no bidi character(U+200E / U+200F / U+202A..=U+202E / U+2066..=U+2069)
- **`pos` field**: non-empty / ≤256 bytes / no control char / no bidi
- **`score` field**: finite(`f32::is_finite`) + non-negative(`>= 0.0`)

ISSUE #94 A03(P2-A 着地時の hardening として未投入だった validation 強化)は P2-B 着地時に同時投入する。具体的には `kotoha-storage::validation` module に `validate_field` / `validate_reading` / `validate_score` の 3 関数を新設し、`UserVocabRecord::insert` / `delete_by_surface_reading` / CLI 層の 3 経路で再利用する。

### 3.10 Pre-PR: P2-A hardening 10 items を別 ISSUE / 別 PR で先行

P2-B kick-off に先立ち、ISSUE #94 P2-B section が列挙する P2-A hardening 10 items を別 ISSUE / 別 PR で先行投入する。10 items は P2-A merge 後に観測された軽微な技術負債(`#[non_exhaustive]` 漏れ補完、validation gap、test fixture 整理等)であり、P2-B コードの上に積むと変更点を分離しづらい。

- **PR 構成**: P2-A hardening pre-PR(0.5 day)→ P2-B docs PR(0.7 day、本 spec)→ P2-B code PR(5.3 day)の 3 段
- **依存関係**: P2-B docs PR は P2-A hardening pre-PR の merge を前提としない(docs のみ変更で衝突しない)。P2-B code PR は P2-A hardening pre-PR の merge を前提とする(validation 投入が前提条件)

### 3.11 Phase 2 spec §5.2 改訂: TSV 第一候補から SQLite 共用 DB へ後付け改訂(Q3 解消)

parent-spec §5.2 は「TSV 第一候補、Phase 5 で JSONL 移行検討」と stub 化していた。本 P2-B docs PR で同節を「SQLite 共用 DB(`kotoha.db`)採用、旧 TSV / JSONL / TOML 比較は ADR 0015 で archived」に後付け改訂する。

- **改訂対象**: parent-spec §3.2 Learning cache / §5.2 persistence / §6.1 既存 trait 拡張 / §8 GUI dict editor / §9 Open questions Q3 の 5 箇所
- **Q3 解消フラグ**: §9 Open questions の Q3「Learning cache 永続化 format」を「**P2-B kickoff brainstorming(2026-04-25、ADR 0015)**」で解消、解消フラグ ✓ を立てる

## 4. アーキテクチャ詳細

### 4.1 crate / module 構成

P2-B は以下のディレクトリ / ファイル構造で実装する。

```text
crates/kotoha-storage/                   (新規 crate)
├── Cargo.toml                           (rusqlite + bundled, kotoha-core 非依存)
├── src/
│   ├── lib.rs                           (pub use re-export)
│   ├── database.rs                      (Database struct + Mutex<Connection>)
│   ├── migration.rs                     (LATEST_VERSION + MIGRATIONS const + apply 関数)
│   ├── user_vocab/
│   │   ├── mod.rs                       (UserVocabStore trait + UserVocabRecord)
│   │   ├── sqlite.rs                    (SqliteUserVocabStore impl)
│   │   └── mock.rs                      (MockUserVocabStore impl, in-memory)
│   ├── learning_cache/
│   │   ├── mod.rs                       (LearningCacheStore trait skeleton, P2-C 実装)
│   │   └── sqlite.rs                    (SqliteLearningCacheStore stub, P2-C 実装)
│   ├── error.rs                         (StorageError enum)
│   ├── path.rs                          (data_dir resolution: KOTOHA_DATA_DIR > XDG)
│   └── validation.rs                    (validate_field / validate_reading / validate_score)
└── migrations/
    └── v001_initial.sql                 (user_vocab + learning_cache schema)

crates/kotoha-core/src/dict/
├── user_vocab.rs                        (新規追加: UserVocab struct + VocabularyLookup impl)
└── (既存 6 modules は変更最小)

crates/kotoha-cli/src/
├── dict_cli.rs                          (新規追加: kotoha-dict subcommand 実装)
└── bin/
    └── dict.rs                          (新規追加: kotoha-dict binary entry point)
```

ディレクトリ命名は P2-A `crates/kotoha-core/src/dict/` と同 pattern を踏襲し、`store.rs` / `mock.rs` / `sqlite.rs` の構造を `user_vocab/` 配下に展開する。

### 4.2 依存方向の図(text)、Clean Architecture DIP

P2-B の依存方向は以下のとおり Clean Architecture DIP に整合する。`kotoha-cli` は `kotoha-core` 経由(IME runtime path)と `kotoha-storage` 直接(CLI subcommand 限定 CRUD)の 2 経路を併用する。図中の `(2) 直接 CRUD` 矢印が後者であり、`kotoha-dict {add, remove, list, show}` subcommand のみが該当する。

```text
kotoha-cli (binary: kotoha-dict)
   │
   │ (1) IME runtime path: depends on
   ▼
kotoha-core (crate)
   ├── dict::user_vocab::UserVocab (impl VocabularyLookup)
   │       │
   │       │ (depends on trait)
   │       ▼
   │   kotoha-storage::user_vocab::UserVocabStore (trait)
   │
   │ (depends on)
   ▼
kotoha-storage (crate)
   ├── user_vocab::SqliteUserVocabStore (impl UserVocabStore)
   ├── learning_cache::LearningCacheStore (trait skeleton, P2-C 実装)
   └── Database (Arc<Mutex<Connection>> + Migration runner)

kotoha-cli ────────────────────────────────► kotoha-storage
   (2) CLI subcommand 限定 CRUD 経路: kotoha-dict {add, remove, list, show} のみ
       Database::open + SqliteUserVocabStore を直接握り、kotoha-core を経由しない
```

依存経路は 2 本である。

- **(1) IME runtime path**: `kotoha-cli → kotoha-core → kotoha-storage`。Phase 3 IBus engine を含む runtime convert 経路はこちらを通り、`UserVocab::lookup` は `kotoha-core::dict::user_vocab` 経由で `Box<dyn UserVocabStore>` を呼ぶ
- **(2) CLI subcommand 限定の直接 CRUD 経路**: `kotoha-cli → kotoha-storage`。`kotoha-dict` subcommand(`add` / `remove` / `list` / `show`)は domain logic を介さない CRUD のみを行うため、`kotoha-core` を経由せず `kotoha-storage::Database::open` + `SqliteUserVocabStore` を直接握る。`kotoha-core` の `VocabularyLookup` trait に `insert` / `delete` を持ち込むのは責務逆転(domain 層が永続化操作を露出)となるため拒否する

依存方向の根拠は以下のとおりである。

- **`kotoha-storage` は `kotoha-core` に逆依存しない**: `kotoha-storage` は永続化詳細層であり、`kotoha-core` の domain layer に依存しない。`UserVocabStore` trait は `kotoha-storage` 側に置き、`kotoha-core::dict::user_vocab::UserVocab` が `Box<dyn UserVocabStore>` を field に保持することで DIP を成立させる
- **`kotoha-cli` は両 crate に依存可**: CLI 層は経路 (1) の lookup と経路 (2) の CRUD の両方を必要とする(`add` / `remove` は CRUD、convert で UserVocab を引く経路は lookup)
- **`rusqlite` C 依存は `kotoha-storage` 1 crate に閉じ込める**: `kotoha-core` 単体 build は `rusqlite` を引かず、Layer 1 unit test / `MockBackend` 経路は C library コンパイル不要

#### 4.2.1 Phase 6+ GUI editor 追加時の経路選択

Phase 6+ で GUI dict editor を追加する際、CLI と同じ pattern で経路 (2) を再利用するか、新たに `kotoha-core` 側に facade(例: `DictEditorFacade`)を設け経路 (1) に統合するかは Phase 6+ design で再検討する(P2-B 範囲外)。原則は CLI と同じく直接 CRUD を選ぶが、GUI が domain 検証(reading 正規化 / 重複検知)を duplicate せざるを得ない場合は facade 採用を検討する。

### 4.3 feature flag 戦略

P2-B は以下 2 個の feature flag を新設する。

| feature flag | 配置 crate | 役割 |
|---|---|---|
| `dict-persist` | `kotoha-core` / `kotoha-cli` | `UserVocab` impl と `kotoha-dict` binary を有効化する。default = [] を維持(ADR 0012 D5 整合) |
| `bundled-sqlite` | `kotoha-storage` 内部 | `rusqlite` の `bundled` feature を有効化する。default で ON とし、distro 同梱 SQLite との衝突回避 |

`kotoha-core/Cargo.toml` に以下を追加する。

```toml
[features]
default = []
dict-persist = ["dep:kotoha-storage", "kotoha-storage/bundled-sqlite", "dict"]
```

`dict-persist` が `dict` を transitively 有効化する理由は、`UserVocab` が `VocabularyLookup` trait(P2-A `dict` feature gate 下で公開)を実装するためである。`dict` 不在のまま `dict-persist` を単独有効化すると trait 不在で compile error となる。

`kotoha-cli/Cargo.toml` に以下を追加する。

```toml
[features]
default = []
dict-persist = ["kotoha-core/dict-persist", "dep:kotoha-storage", "dep:clap"]

[[bin]]
name = "kotoha-dict"
required-features = ["dict-persist"]
```

ADR 0012 D5「default = []」厳守方針との整合上、`kotoha-dict` binary は opt-in でのみビルドされる。Phase 1 / P2-A の baseline build(default features)は P2-B 投入後も `kotoha-storage` を引かない。

## 5. Schema(SQLite v001)

P2-B が同梱する初版 schema(`migrations/v001_initial.sql`)を以下に示す。`user_vocab` table は P2-B で本格使用、`learning_cache` table は schema only(P2-C で本格使用)である。

### 5.1 `user_vocab` table

```sql
CREATE TABLE user_vocab (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    surface     TEXT    NOT NULL,
    reading     TEXT    NOT NULL,
    pos         TEXT    NOT NULL DEFAULT '名詞-固有名詞-一般',
    score       REAL    NOT NULL DEFAULT 1.0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    UNIQUE(surface, reading)
);

CREATE INDEX idx_user_vocab_reading ON user_vocab(reading);
```

各 field の意図:

- **`id`**: AUTOINCREMENT を採用し、`remove <id>` での指定削除を可能にする(連続値ではなく単調増加 id)
- **`surface`**: ユーザが確定した漢字交じり表記(`String`、≤256 bytes、§9 Validation)
- **`reading`**: hiragana canonical 表記(§3.3)、prefix 検索のため `idx_user_vocab_reading` を貼る
- **`pos`**: 品詞情報、default `名詞-固有名詞-一般`(SudachiDict-core の品詞体系に揃える)
- **`score`**: lookup 時の score(`f32`、finite + non-negative、§9 Validation)、default `1.0` は P2-A `CustomVocab` の score 同値に揃える(§3.7 lookup priority と整合)
- **`created_at` / `updated_at`**: UNIX epoch seconds(`i64`)、`updated_at` は P2-B では `created_at` と同値(update subcommand 不在のため、§3.8)
- **`UNIQUE(surface, reading)`**: 同 (surface, reading) の重複登録を reject(§2 Out of scope の `pos` 違い多重保持非対応と整合)

### 5.2 `learning_cache` table(schema only、P2-B では未使用)

```sql
CREATE TABLE learning_cache (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kana_input   TEXT    NOT NULL,
    chosen_kanji TEXT    NOT NULL,
    frequency    INTEGER NOT NULL DEFAULT 1,
    last_used_at INTEGER NOT NULL,
    UNIQUE(kana_input, chosen_kanji)
);

CREATE INDEX idx_learning_cache_kana ON learning_cache(kana_input);
```

各 field の意図は parent-spec §5.1 と同一である。P2-B では `idx_learning_cache_kana` のみを貼り、`idx_learning_cache_last_used`(LRU eviction の `ORDER BY last_used_at` 用)は v002 schema(P2-C で追加予定)で導入する。

### 5.3 PRAGMA 設定

`Database::open` 時に以下の PRAGMA を実行する。

```sql
PRAGMA journal_mode = WAL;       -- Write-Ahead Logging で並行 read 性能と crash 安全性を確保
PRAGMA synchronous  = NORMAL;    -- WAL 前提で fsync 頻度を down、性能と耐久性のバランス
PRAGMA foreign_keys = ON;        -- v002 以降の外部キー制約を有効化(v001 では無効化と等価)
PRAGMA temp_store   = MEMORY;    -- 一時テーブル / インデックスを RAM に置き、IO 削減
```

WAL モード採用により `kotoha-dict` CLI 実行中の Phase 3 IBus engine による read は block されず、`-wal` / `-shm` ファイルが `kotoha.db` 隣接に生成される。

### 5.4 DB 配置パス

DB 配置パスは以下 8 ステップで resolve / 検証する(`kotoha-storage::path::resolve_data_dir`)。`canonicalize` 単独では symlink を経由した system path 配下への traversal を防げないため(CWE-59 / CWE-367)、prefix containment と system path blacklist の二段検証 + 作成 ↔ canonicalize の順序を明示する。

#### 5.4.1 path resolution + 検証アルゴリズム

1. **target_dir 解決**: 環境変数を以下の優先順で resolve する
   - `KOTOHA_DATA_DIR` が設定されていれば、その値を target_dir(明示指定 path)とする
   - 未設定時は `XDG_DATA_HOME` が設定されていれば `$XDG_DATA_HOME/kotoha`
   - 上記いずれも未設定時は `$HOME/.local/share/kotoha`(XDG Base Directory 仕様 default)
2. **prefix containment 検証(canonicalize 前)**: target_dir の prefix が以下のいずれかの allowlist 配下であることを `Path::starts_with` で検証する
   - `KOTOHA_DATA_DIR` で明示指定された場合は、その明示 path 配下のみ許容
   - `XDG_DATA_HOME` 経由の場合は、`$XDG_DATA_HOME` 配下のみ許容
   - default `$HOME/.local/share/kotoha` 経由の場合は、`$HOME` 配下のみ許容
3. **system path blacklist(canonicalize 前)**: target_dir の prefix が以下 system path のいずれかに該当する場合は `StorageError::InvalidPath` で reject する
   - `/etc`, `/var`, `/tmp`, `/proc`, `/sys`, `/dev`, `/root`, `/boot`
4. **dir 作成**: target_dir が存在しない場合は `std::fs::create_dir_all(&target_dir)` で再帰的に作成する(canonicalize **前**に作成、存在しない path は canonicalize できないため)
5. **canonicalize 適用**: 作成後の target_dir に対し `canonicalize()` を適用し、symlink を解決した絶対 path を取得する
6. **prefix containment + blacklist 再検証(canonicalize 後)**: canonicalize 後 path に対しステップ 2 / 3 と同じ検証を再実行する。symlink 解決後に system path 配下に飛ぶケース(symlink trick による traversal)を defense-in-depth で reject する
7. **絶対 path assert**: `path.is_absolute()` を assert する(canonicalize 後は常に絶対 path となるが invariant として保持)
8. **DB ファイル size sanity warn**: 既存 `kotoha.db` の size が 100 MiB を超えた場合は stderr に warn を出力する(LearningCache LRU 上限 10,000 + UserVocab 数千件 = 期待値 1〜10 MiB を大幅超過する状況の早期検知)、exit code は 0 を維持

#### 5.4.2 検証順序の根拠

ステップ 2 / 3 の事前検証(canonicalize 前)とステップ 6 の事後検証(canonicalize 後)を二段で実施する根拠は、CWE-59(symlink following)+ CWE-367(TOCTOU)に対する defense-in-depth である。事前検証のみでは、`$HOME/.local/share/kotoha` 配下に作成された symlink が `/etc/kotoha` を指す場合に bypass される。事後検証のみでは、canonicalize で root を超える traversal(`../../../etc`)が ext4 / btrfs file system で先に解決されるパターンに対し、target_dir 解決段階で reject できない。

#### 5.4.3 `StorageError::InvalidPath` variant の追加

§9.4 の `StorageError` enum に `InvalidPath { path: PathBuf, reason: String }` variant を追加し、ステップ 2 / 3 / 6 の reject 経路から共通で返す(§9.4 の error mapping 表に同 variant を追記する)。

## 6. Trait API と型

### 6.1 `UserVocabStore` trait

`kotoha-storage::user_vocab::UserVocabStore` は UserVocab の永続化抽象境界を提供する。

```rust
/// User-managed vocabulary store の抽象境界。
///
/// # Preconditions
/// - `reading` は hiragana canonical 表記(`U+3040..=U+309F + U+30FC + U+30FB`)
/// - `surface` / `pos` は non-empty / ≤256 bytes / no control char / no bidi
/// - `score` は finite + non-negative
///
/// # Postconditions
/// - `find_by_reading` は score 降順で最大 `limit` 件返す
/// - `insert` 成功時は `id` を返す
/// - UNIQUE(surface, reading) 違反は [`StorageError::DuplicateEntry`]
///
/// # Errors
/// - [`StorageError::InvalidField`] when validation fails
/// - [`StorageError::DuplicateEntry`] on UNIQUE conflict
/// - [`StorageError::NotFound`] on delete miss
/// - [`StorageError::Sqlite`] on backend failure
pub trait UserVocabStore: Send + Sync {
    fn find_by_reading(&self, reading: &str, limit: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
    fn list_all(&self, limit: usize, offset: usize) -> Result<Vec<UserVocabRecord>, StorageError>;
    fn insert(&self, record: UserVocabRecord) -> Result<i64, StorageError>;
    fn delete_by_id(&self, id: i64) -> Result<(), StorageError>;
    fn delete_by_surface_reading(&self, surface: &str, reading: &str) -> Result<(), StorageError>;
}
```

### 6.2 `UserVocabRecord` 構造体

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct UserVocabRecord {
    pub id:         Option<i64>,  // insert 前は None、find / list は Some
    pub surface:    String,
    pub reading:    String,
    pub pos:        String,
    pub score:      f32,
    pub created_at: i64,          // UNIX epoch seconds
    pub updated_at: i64,
}
```

### 6.3 `LearningCacheStore` trait skeleton(P2-B では interface のみ確定)

P2-B は `LearningCacheStore` trait の signature のみを確定し、実装本体は P2-C で行う。

```rust
/// Learning cache store の抽象境界(P2-C で実装)。
pub trait LearningCacheStore: Send + Sync {
    fn lookup(&self, kana_input: &str, limit: usize) -> Result<Vec<LearningCacheRecord>, StorageError>;
    fn record_choice(&self, kana_input: &str, chosen_kanji: &str) -> Result<(), StorageError>;
    fn evict_lru(&self, max_entries: usize) -> Result<usize, StorageError>;
}

#[derive(Debug, Clone)]
pub struct LearningCacheRecord {
    pub id:           i64,
    pub kana_input:   String,
    pub chosen_kanji: String,
    pub frequency:    u32,
    pub last_used_at: i64,
}
```

P2-B は `SqliteLearningCacheStore` を unimplemented stub として配置し、P2-C で本実装に置換する(table schema は v001 で同梱済のため migration 不要)。

### 6.4 `Database` 構造体

`Database` は `Arc` で wrap された共有 ownership 構造体であり、`UserVocabStore` 系 / `LearningCacheStore` 系の複数 Store が同一 `Database` instance を `Arc::clone` で共有する。`open` の戻り値型は `Arc<Database>` とし、CLI の factory(§8.2)が `Box<dyn UserVocabStore>` / `Box<dyn LearningCacheStore>` に詰める際の lifetime 衝突を回避する。

```rust
pub struct Database {
    conn: std::sync::Mutex<rusqlite::Connection>,
}

impl Database {
    /// `path` の SQLite DB を open し、未適用 migration を apply する。
    ///
    /// # Postconditions
    /// - 戻り値は `Arc<Database>` で返り、複数 Store(`UserVocab` / `LearningCache`)が
    ///   `Arc::clone` で同一 `Database` instance を共有 ownership で保持する
    /// - `path` の親ディレクトリが存在しない場合は再帰的に作成
    /// - PRAGMA journal_mode / synchronous / foreign_keys / temp_store を設定
    /// - `PRAGMA user_version` を読み取り、`MIGRATIONS` の未適用分を順次 apply
    pub fn open(path: &std::path::Path) -> Result<std::sync::Arc<Self>, StorageError> { /* ... */ }

    /// `Arc<Database>` を `SqliteUserVocabStore` で wrap し owned `Box<dyn UserVocabStore>`
    /// として返す。戻り値は lifetime parameter を持たないため、factory(§8.2)の local
    /// 変数で Database を保持しながら `Box<dyn UserVocabStore>` を `Vec` に詰める経路で
    /// borrow checker と衝突しない。
    pub fn user_vocab_store(self: &std::sync::Arc<Self>) -> Box<dyn UserVocabStore> {
        Box::new(SqliteUserVocabStore { db: std::sync::Arc::clone(self) })
    }

    /// `Arc<Database>` を `SqliteLearningCacheStore` で wrap した owned `Box<dyn
    /// LearningCacheStore>` を返す(P2-B では unimplemented stub、P2-C で本実装)。
    pub fn learning_cache_store(self: &std::sync::Arc<Self>) -> Box<dyn LearningCacheStore> {
        Box::new(SqliteLearningCacheStore { db: std::sync::Arc::clone(self) })
    }
}

pub struct SqliteUserVocabStore {
    db: std::sync::Arc<Database>,
}

impl UserVocabStore for SqliteUserVocabStore { /* Arc<Database> 経由で Mutex<Connection> を借りる */ }
```

#### 6.4.1 共有 ownership の Postcondition

- **`Database` の内部状態**: `Database` は内部で `Mutex<rusqlite::Connection>` を 1 個保持し、SQLite Connection の `!Sync` 制約を thread 跨ぎで補完する
- **複数 Store の共有 ownership**: `Arc<Database>` を `SqliteUserVocabStore` / `SqliteLearningCacheStore` の双方が field として保持し、同一 Connection に対する逐次 lock を `Arc::clone` 経由で共有する(2 store 跨ぎの ATOMIC transaction が同一 Connection 上で成立、ADR 0015 R3 と整合)
- **lifetime parameter 不在**: `user_vocab_store` / `learning_cache_store` の戻り値は `'a` lifetime を持たない `Box<dyn ... >`(owned)であり、factory の local 変数 scope で Database を生かしたまま `Box<dyn UserVocabStore>` を `Vec` に push する経路で borrow checker と衝突しない

`Mutex<Connection>` は `rusqlite::Connection` が `!Sync` であるための単純対応である。Phase 3 IBus engine は別プロセスで動作するため、この Mutex は同一プロセス内 thread 競合のみを保護する(プロセス間競合は SQLite の WAL モードが扱う)。

### 6.5 Migration runner

```rust
pub const LATEST_VERSION: i32 = 1;

pub const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../migrations/v001_initial.sql")),
];

pub fn apply_migrations(conn: &rusqlite::Connection) -> Result<(), StorageError> {
    let current: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > current) {
        conn.execute_batch(sql)?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", version))?;
    }
    Ok(())
}
```

`include_str!` でバイナリに同梱するため、distribution 時に migrations ディレクトリを別配布する必要はない。

### 6.6 `MockUserVocabStore`(test 用 in-memory)

```rust
pub struct MockUserVocabStore {
    records: std::sync::Mutex<Vec<UserVocabRecord>>,
    next_id: std::sync::Mutex<i64>,
}

impl UserVocabStore for MockUserVocabStore { /* in-memory CRUD */ }
```

P2-A `MockEngine` / `MockVocab` と同 pattern で配置し、Layer 2 integration test で `kotoha-core::dict::user_vocab::UserVocab` に注入可能とする(SQLite 不在環境でも test 実行可能)。

### 6.7 `kotoha-core::dict::user_vocab::UserVocab` の `VocabularyLookup` impl pattern

```rust
pub struct UserVocab {
    store:    Box<dyn UserVocabStore>,
    vocab_id: String,
}

impl UserVocab {
    pub fn new(store: Box<dyn UserVocabStore>) -> Self {
        Self { store, vocab_id: "user-vocab".to_string() }
    }
}

impl VocabularyLookup for UserVocab {
    fn lookup(&self, reading: &str) -> Vec<VocabEntry> {
        self.store.find_by_reading(reading, 32)
            .unwrap_or_default()
            .into_iter()
            .map(|r| VocabEntry {
                surface: r.surface,
                reading: r.reading,
                pos:     r.pos,
                score:   r.score,
            })
            .collect()
    }
    fn vocab_id(&self) -> &str { &self.vocab_id }
}
```

`unwrap_or_default()` 採用理由: `VocabularyLookup::lookup` の signature(`-> Vec<VocabEntry>`、Result を返さない)に整合させるため、SQLite backend エラー時は空 Vec を返し、上位の `DictionaryBackend::convert` の MorphologicalEngine 経路と CustomVocab 経路で recall を維持する。

## 7. CLI 仕様

### 7.1 `kotoha-dict` binary とグローバルフラグ

`kotoha-dict` は `kotoha-cli` crate の `[[bin]]` として配置し、`--features dict-persist` で有効化する。

```text
kotoha-dict [GLOBAL FLAGS] <SUBCOMMAND> [ARGS]

Global flags:
  --data-dir <PATH>   DB 配置ディレクトリを上書き(KOTOHA_DATA_DIR と同等、CLI 引数優先)
  --quiet             成功時の確認メッセージを抑止(`add` / `remove` で使用)
  --json              出力を JSON 形式で返す(`list` / `show` で使用、機械読取り向け)

Subcommands:
  add     <surface> <reading> [--pos POS] [--score SCORE]
  remove  <id> | --surface <S> --reading <R>
  list    [--format text|json] [--reading <PREFIX>] [--limit N] [--offset N]
  show    <id>
```

### 7.2 `add` subcommand

```text
kotoha-dict add <SURFACE> <READING> [--pos POS] [--score SCORE]
```

| 引数 / フラグ | 必須 | default | 内容 |
|---|---|---|---|
| `<SURFACE>` | 必須 | — | 漢字交じり表記(non-empty / ≤256 bytes) |
| `<READING>` | 必須 | — | hiragana 読み(全 hiragana / 全 ASCII / 混在 reject) |
| `--pos POS` | 任意 | `名詞-固有名詞-一般` | 品詞 |
| `--score SCORE` | 任意 | `1.0` | lookup score(finite + non-negative) |

`<READING>` は §3.3 の auto-detect 仕様に従う。全 ASCII 入力時は `kotoha-core::romaji::RomajiConverter` で hiragana に変換した後 DB に保存する。

成功時の出力(`--quiet` 未指定):

```text
added: id=42 surface="日野岡" reading="ひのおか" pos="名詞-固有名詞-人名" score=1.0
```

### 7.3 `remove` subcommand

```text
kotoha-dict remove <ID>
kotoha-dict remove --surface <SURFACE> --reading <READING>
```

`<ID>` 指定と `--surface` + `--reading` 指定は排他 group(clap `ArgGroup` 経由)。両方指定 / 両方未指定は exit code 2(入力エラー)。

成功時の出力:

```text
removed: id=42
```

### 7.4 `list` subcommand

```text
kotoha-dict list [--format text|json] [--reading <PREFIX>] [--limit N] [--offset N]
```

| フラグ | default | 内容 |
|---|---|---|
| `--format` | `text` | 出力形式(`text` / `json`) |
| `--reading` | なし | reading の prefix match(SQL `reading LIKE 'PREFIX%'`)、§2 Out of scope の通り fuzzy 検索ではない |
| `--limit` | `100` | 返却最大件数 |
| `--offset` | `0` | スキップ件数 |

text format 出力例:

```text
ID    SURFACE      READING        POS                              SCORE
42    日野岡       ひのおか       名詞-固有名詞-人名               1.0
43    琴葉         ことば         名詞-固有名詞-人名               1.0
```

JSON format 出力例(`--format json` または global `--json`):

```json
[
  {"id":42,"surface":"日野岡","reading":"ひのおか","pos":"名詞-固有名詞-人名","score":1.0,"created_at":1745529600,"updated_at":1745529600},
  {"id":43,"surface":"琴葉","reading":"ことば","pos":"名詞-固有名詞-人名","score":1.0,"created_at":1745529700,"updated_at":1745529700}
]
```

### 7.5 `show` subcommand

```text
kotoha-dict show <ID>
```

text format 出力例:

```text
id:         42
surface:    日野岡
reading:    ひのおか
pos:        名詞-固有名詞-人名
score:      1.0
created_at: 2026-04-25T00:00:00Z (1745529600)
updated_at: 2026-04-25T00:00:00Z (1745529600)
```

### 7.6 exit code 体系

| exit code | 意味 | 例 |
|---|---|---|
| 0 | 成功 | `add` / `remove` / `list` / `show` の正常終了 |
| 1 | 内部エラー | DB IO error、migration 失敗、`StorageError::Sqlite` |
| 2 | 入力エラー | 引数不足、`<READING>` の混在 reject、`--surface` と `<id>` の同時指定 |
| 3 | 重複 | UNIQUE(surface, reading) 違反、`StorageError::DuplicateEntry` |
| 4 | not found | `remove <id>` / `show <id>` で entry 不在、`StorageError::NotFound` |

### 7.7 `<READING>` auto-detect 仕様

CLI 層は `<READING>` 引数を以下の規則で normalize する。

1. 全文字が hiragana(U+3040..=U+309F + U+30FC + U+30FB)→ そのまま使用
2. 全文字が ASCII(U+0020..=U+007E、半角英数記号)→ `RomajiConverter::romaji_to_hiragana` で変換
3. 上記 2 通り以外(混在、カタカナ混入、全角英字混入等)→ exit code 2 で reject、stderr に「READING must be all hiragana or all ASCII romaji」

normalize 後の reading は §9 Validation の「hiragana + ー + ・ のみ」を満たすことを DB 層で再検証する。

## 8. DictionaryBackend 統合

### 8.1 `DictionaryConfig.user_vocab_db_path: Option<PathBuf>` 追加

P2-A の `DictionaryConfig` 構造体に新 field `user_vocab_db_path: Option<PathBuf>` を追加する。`#[non_exhaustive]` 属性は P2-A で既に付与済のため、本追加は ADR 0011 D2 / ADR 0014 D7 と整合する非破壊変更である。

```rust
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DictionaryConfig {
    pub system_dict_path:     PathBuf,
    pub custom_vocab_path:    Option<PathBuf>,
    pub user_vocab_db_path:   Option<PathBuf>,  // P2-B 追加
}
```

### 8.2 `load_backend` factory 拡張

`#[cfg(feature = "dict-persist")]` gate 下で、`load_backend` factory の `BackendConfig::Dictionary` arm が `user_vocab_db_path` の `Some(path)` を検出した場合に `UserVocab` を構築して `Vec<Box<dyn VocabularyLookup>>` に追加する。`Database::open` は `Arc<Database>` を返し、`db.user_vocab_store()` は owned `Box<dyn UserVocabStore>` を返すため(§6.4)、factory の local 変数 `db` を `Arc::clone` 経由で生かしたまま `Box` を `Vec` に push する経路で borrow checker と衝突しない。

```rust
#[cfg(feature = "dict-persist")]
BackendConfig::Dictionary { config } => {
    let mut vocabs: Vec<Box<dyn VocabularyLookup>> = Vec::new();
    if let Some(path) = &config.custom_vocab_path {
        vocabs.push(Box::new(CustomVocab::load(path)?));
    }
    if let Some(db_path) = &config.user_vocab_db_path {
        let db: std::sync::Arc<kotoha_storage::Database> =
            kotoha_storage::Database::open(db_path)?;
        let store: Box<dyn UserVocabStore> = db.user_vocab_store();
        vocabs.push(Box::new(UserVocab::new(store)));
        // db (Arc<Database>) は scope 終了で drop されるが、
        // store 内部の Arc<Database> が Arc::clone で生存し続けるため Connection は閉じない
    }
    let engine = Box::new(SudachiAdapter::load(&config.system_dict_path)?);
    Ok(Box::new(DictionaryBackend::new(engine, vocabs)))
}
```

### 8.3 順序 `[CustomVocab, UserVocab]` の根拠

`vocabs` Vec への push 順序は §3.7 で確定したとおり `[CustomVocab, UserVocab]` を厳守する。`score_sort_dedupe` helper は score 降順で安定 sort し、score 同値時は元の Vec の順序を保つため、`Vec` 前方の CustomVocab が score tie で勝ち残る。

### 8.4 既存 P2-A 175 / 223 baseline 退行禁止条件

P2-B 投入後も以下の baseline を退行させない。

- **default features build**: `cargo test --workspace` で 175 PASS を維持(`kotoha-storage` を引かない経路)
- **dict feature build**: `cargo test --workspace --features kotoha-core/dict` で 223 PASS を維持(P2-A 確定値、UserVocab 無効経路)
- **dict-persist feature build(P2-B 追加)**: `cargo test --workspace --features kotoha-core/dict-persist` で 223 + α PASS(α は §10 で見積る新規 test 件数)

退行検出は lefthook pre-push gate で workspace 全 test 実行により行う。

## 9. Validation

### 9.1 field 共通制約

`kotoha-storage::validation::validate_field(name: &str, value: &str) -> Result<(), StorageError>` は以下を検査する。

- **non-empty**: `value.is_empty()` を reject
- **byte size**: `value.len() > 256` を reject(UTF-8 byte 換算)
- **no control char**: `c.is_control()` で `\t`(U+0009)/`\n`(U+000A)/`\r`(U+000D)も reject(SQL injection / TSV 互換性 / log integrity の 3 観点)
- **no bidi character**: U+200E / U+200F / U+202A..=U+202E / U+2066..=U+2069 を reject(`Trojan Source` 攻撃緩和、CVE-2021-42574 整合)
- **no PUA char(Phase 5 allowlist 例外あり)**: U+E000..=U+F8FF(BMP PUA)および U+F0000..=U+10FFFD(supplementary PUA)を reject、ただし **Phase 5 Mixed JP/EN allowlist (U+EE00..=U+EE03) のみ例外として許容** する(Karukan reference 互換、memory `project_mixed_jp_en_phase5_goal` 参照)
- **no Variation Selector**: U+FE00..=U+FE0F(VS-1〜VS-16)を reject
- **no Tag character**: U+E0000..=U+E007F を reject

`surface` / `reading` / `pos` の 3 field 全てに本共通制約を適用する(`reading` は §9.2 で hiragana-only 制約をさらに重畳)。

#### 9.1.1 判定 helper

```rust
/// PUA(Private Use Area)を reject する判定。Phase 5 Mixed JP/EN
/// allowlist (U+EE00..=U+EE03) は許容する。
fn is_disallowed_pua(c: char) -> bool {
    let cp = c as u32;
    let is_pua = (0xE000..=0xF8FF).contains(&cp) || (0xF0000..=0x10FFFD).contains(&cp);
    let is_phase5_allowlist = (0xEE00..=0xEE03).contains(&cp);
    is_pua && !is_phase5_allowlist
}

fn is_variation_selector(c: char) -> bool {
    (0xFE00..=0xFE0F).contains(&(c as u32))
}

fn is_tag_char(c: char) -> bool {
    (0xE0000..=0xE007F).contains(&(c as u32))
}
```

#### 9.1.2 PUA / VS / Tag reject の根拠(Phase 5 trust boundary)

Phase 5 KotohaNative model(ADR 0010 / Phase 5 spec)では、UserVocab の `surface` / `reading` / `pos` 内容が prompt 構築経路に流れ、custom model の context として model 入力に到達する。PUA / Variation Selector / Tag character は visual に invisible / 無害に見える一方、Karukan-style PUA tokens(memory `project_karukan_reference_architecture`)を悪用する prompt-injection 経路となり得る(LLM が PUA tokens を special token として解釈する可能性)。本 ADR 0015 / 本 spec の design では、3 char class を `validate_field` で reject し、Phase 5 Mixed JP/EN 用 allowlist (U+EE00..=U+EE03) のみを Karukan 互換の特例として許容する(allowlist の 4 codepoint は Phase 5 spec で意味を再確認する)。

### 9.2 reading 追加制約

`kotoha-storage::validation::validate_reading(value: &str) -> Result<(), StorageError>` は §9.1 共通制約に加え以下を適用する。

- **hiragana only**: 全文字が U+3040..=U+309F + U+30FC(長音「ー」) + U+30FB(中黒「・」) のいずれか
- **CLI normalize 後でも DB 層で再検証**: CLI 層の auto-detect(§7.7)後でも、`UserVocabStore::insert` 直前に `validate_reading` を呼び、DB 直書込み経路(`assert_cmd` 以外の test 経路)でも contract を維持

### 9.3 score 制約

`kotoha-storage::validation::validate_score(value: f32) -> Result<(), StorageError>` は以下を検査する。

- **finite**: `value.is_finite()`、NaN / Infinity / -Infinity を reject
- **non-negative**: `value >= 0.0` を要求(score 降順 sort の前提を保護)

### 9.4 error mapping

violation 発生時は `StorageError::InvalidField { name: String, reason: String }` を返す。path resolution(§5.4)の reject 経路は `StorageError::InvalidPath { path: PathBuf, reason: String }` を返す。

```rust
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("invalid field {name}: {reason}")]
    InvalidField { name: String, reason: String },
    #[error("invalid path {path:?}: {reason}")]
    InvalidPath { path: std::path::PathBuf, reason: String },
    #[error("duplicate entry: surface={surface} reading={reading}")]
    DuplicateEntry { surface: String, reading: String },
    #[error("entry not found")]
    NotFound,
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("migration error: {0}")]
    Migration(String),
}
```

`StorageError::InvalidField` の `reason` 列挙は §9.1 / §9.2 / §9.3 の violation 種別を文字列化する。具体的には「empty」「>256 bytes」「control char (NUL/TAB/NL/CR)」「bidi character」「PUA char (excluding U+EE00..=U+EE03)」「Variation Selector」「Tag char」「non-hiragana」「NaN」「Infinity」「negative score」を `reason` 値として採用する(§9.5 参照)。

CLI 層は `StorageError` variant に応じて exit code(§7.6)を mapping する。`StorageError::InvalidPath` は exit code 1(内部エラー扱い、§7.6)に mapping する。

### 9.5 validation reason 列挙(`InvalidField::reason`)

`StorageError::InvalidField` の `reason` field は以下の固定文字列のいずれかを採用する。test 層と CLI 層が string match で violation 種別を判別できる安定 vocabulary として確定する。

| reason 値 | 検出元 |
|---|---|
| `empty` | §9.1 non-empty |
| `byte size > 256` | §9.1 byte size |
| `control char` | §9.1 no control char |
| `bidi character` | §9.1 no bidi |
| `PUA char` | §9.1 no PUA(allowlist 例外を除く) |
| `Variation Selector` | §9.1 no VS |
| `Tag char` | §9.1 no Tag char |
| `non-hiragana reading` | §9.2 hiragana only |
| `score not finite` | §9.3 finite |
| `score negative` | §9.3 non-negative |

## 10. Test 戦略

### 10.0 Validation test の責任分界(Layer 1 / Layer 3)

§9 で validation を `kotoha-storage::validation` module に一元化する設計に対し、Layer 1(storage unit)と Layer 3(CLI integration)のどちらが各 invariant を負担するかを以下 table で明示する。Layer 1 が全 invariant を網羅的に test し、Layer 3 は CLI 経路の reject 分岐(exit code mapping)を代表 1 件のみ確認することで、二重 validation を defense-in-depth として担保する。

| Validation 観点 | Layer 1(storage unit) | Layer 3(CLI integration) |
|---|---|---|
| field empty / >256 byte | ✓(全 3 field を網羅) | ✓(代表 1 件、全件は Layer 1 で担保) |
| control char(NUL / TAB / NL / CR) | ✓ | ✓(代表 1 件) |
| bidi controls(U+200B-U+200F、U+202A-U+202E、U+2066-U+2069) | ✓ | ✓(代表 1 件) |
| non-hiragana reading | ✓ | ✓(romaji auto-detect 通過後の失敗 path を含む) |
| score NaN / Infinity / 負値 | ✓ | ✓(`--score nan` 等の代表 case) |
| PUA / VS / Tag char(§9.1) | ✓(Phase 5 allowlist U+EE00-EE03 例外を含む) | ✓(代表 1 件) |
| path traversal(symlink / blacklist、§5.4) | ✓(`path::resolve_data_dir` の unit test) | — |

責任分界の方針: Layer 1 は全 invariant を網羅し reject contract を確定する。Layer 3 は CLI 経路から storage 層への validation bypass が起きないことを代表 1 件で確認するに留め、invariant 列挙は Layer 1 に委ねる(test 件数の爆発を防止しつつ、CLI 経路独自の reject branch / exit code mapping を確実に検証する)。

### 10.1 Layer 1: `kotoha-storage` unit test

`kotoha-storage` crate 内の `#[cfg(test)]` で 26〜28 件程度実装する。

| 対象 | 件数目安 | 内容 |
|---|---|---|
| `MockUserVocabStore` の CRUD | 6〜8 | insert / find_by_reading / list_all / delete_by_id / delete_by_surface_reading |
| `SqliteUserVocabStore` を `:memory:` で実行 | 6〜8 | `Connection::open_in_memory` で migration apply 後 CRUD |
| `Database::open` migration | 3〜4 | 空 DB → v001 適用、既に v001 適用済 → no-op、PRAGMA 設定検証 |
| **`MIGRATIONS` 配列 invariant**(必須) | 3 | (a) version 昇順 invariant、(b) 中間 version skip-and-resume、(c) idempotency。詳細は §10.1.1 |
| `validation` module | 3〜4 | hiragana-only reject、bidi char reject、PUA / VS / Tag char reject(Phase 5 allowlist 例外含む)、score finite / non-negative |
| `path::resolve_data_dir` | 2〜3 | system path blacklist reject、$HOME prefix containment、symlink 解決後の事後検証(§5.4) |

`:memory:` SQLite 採用により外部ファイル不要で並行実行可能。

#### 10.1.1 `MIGRATIONS` 配列 invariant test(必須 3 項目)

将来の v002 / v003 追加時の order regression を構造的に予防するため、以下 3 件を Layer 1 unit test の必須項目とする。P2-B 着地時点の `MIGRATIONS` 長は 1(v001 のみ)だが、3 件全てを future-proof として実装する。

1. **version 昇順 invariant**: `MIGRATIONS` const 配列の version 列が strict 昇順で並ぶことを以下で確認する
   ```rust
   #[test]
   fn migrations_are_strictly_ascending() {
       assert!(
           MIGRATIONS.windows(2).all(|w| w[0].0 < w[1].0),
           "MIGRATIONS must be in strictly ascending version order"
       );
   }
   ```
2. **中間 version skip-and-resume**: `PRAGMA user_version = N`(N は中間値、例えば v003 schema が存在する状況で N=2)から最新まで未適用 migration が順次 apply されることを `:memory:` Connection で検証する。P2-B では `MIGRATIONS` 長が 1 のため、N=0 → v001 apply → N=1 で完了する 1 ケースを実装し、v002 追加時に skip-and-resume を本格 test する基盤を先出しする
3. **idempotency**: 既に最新 version の DB を `Database::open` で再 open しても no-op であることを、`apply_migrations` の戻り値 / `PRAGMA user_version` の値が `LATEST_VERSION` で一致することで検証する。具体的には同一 `:memory:` Connection で 2 回 `apply_migrations` を呼び、2 回目が SQL を execute せず `user_version == LATEST_VERSION` を維持することを assert する

### 10.2 Layer 2: `kotoha-core::dict::user_vocab` integration test

`crates/kotoha-core/tests/dict_user_vocab.rs` に integration test を 6〜8 件実装する。

| 対象 | 内容 |
|---|---|
| `UserVocab::new` + `MockUserVocabStore` | trait 経由で `VocabularyLookup::lookup` が score 降順で返す |
| `lookup` 空 store | 未 hit で empty Vec |
| `lookup` SQLite backend エラー時 | empty Vec を返し panic しない |
| 複数 entry の score 降順 | MockUserVocabStore に複数 entry を仕込んだ場合の order 検証 |

`MockUserVocabStore` 注入により SQLite 不在環境でも実行可能。

### 10.3 Layer 3: `kotoha-cli::dict_cli` integration test

`crates/kotoha-cli/tests/dict_cli.rs` に `assert_cmd` ベース integration test を 10〜14 件実装する。

#### 10.3.1 test 戦略(env var race condition 回避)

`KOTOHA_DATA_DIR` 環境変数経由の path 注入は process-global であり、`std::env::set_var` を `#[test]` 並列実行下で複数 thread が同時呼出した場合に race condition を起こす。`tempfile::tempdir` で per-test 物理 path を分離しても、env var 自体の race は防げない。本 test 戦略は以下 3 点を必須要件とする。

- **`serial_test::serial` annotation 必須**: `KOTOHA_DATA_DIR` env var を test 内で操作する全 case は `#[serial]` を必須付与する。`cargo test` の thread-pool 並列実行下でも、`#[serial]` annotation 付き test 群は順次実行されるため env var 操作が race しない
- **`tempfile::tempdir` で物理 path 分離**: per-test で `TempDir` を作成し、`KOTOHA_DATA_DIR` を `tempdir.path()` に向けることで test 間の DB ファイル干渉を防ぐ
- **`Database::open` を実 file で**: `:memory:` ではなく実 file で `Database::open` を呼び、CLI subcommand の end-to-end 経路(WAL ファイル生成、PRAGMA 設定、symlink 解決)を実機相当で検証する

dev-dependency 前提として `serial_test` + `tempfile` の 2 crate が `kotoha-cli/Cargo.toml` の `[dev-dependencies]` に追加されていることを要する(§3.10 / §10.6 参照)。

| 対象 | 内容 |
|---|---|
| `kotoha-dict add` 正常 | tempfile DB に entry 追加、exit code 0 |
| `kotoha-dict add` 重複 | UNIQUE 違反、exit code 3 |
| `kotoha-dict add` 不正 reading | 混在 reject、exit code 2 |
| `kotoha-dict add --score nan` | finite reject、exit code 2 |
| `kotoha-dict remove <id>` 正常 | 削除成功、exit code 0 |
| `kotoha-dict remove <id>` 不在 | not found、exit code 4 |
| `kotoha-dict remove --surface --reading` | 排他 group 検証 |
| `kotoha-dict list` 全件 | text format 出力検証 |
| `kotoha-dict list --format json` | JSON parse 可能 |
| `kotoha-dict list --reading <PREFIX>` | prefix match 動作 |
| `kotoha-dict show <id>` 正常 / 不在 | exit code 0 / 4 |

`tempfile::tempdir` で DB ディレクトリを test ごとに分離し、`KOTOHA_DATA_DIR` 経由で path 注入する。

### 10.4 Layer 4: `DictionaryBackend` end-to-end test

`crates/kotoha-core/tests/dict_backend_user_vocab.rs` に `--features dict-persist` 下の end-to-end test を 4〜6 件実装する。

| 対象 | 内容 |
|---|---|
| `BackendConfig::Dictionary { user_vocab_db_path: Some(path) }` | UserVocab 込みの `convert` が UserVocab entry を返す |
| `[CustomVocab, UserVocab]` 順序 | score tie 時に CustomVocab 由来 entry が勝ち残る |
| `user_vocab_db_path: None` | UserVocab 無効、P2-A baseline と同等動作 |
| feature gate 無効時 | `KanjiError::FeatureDisabled { feature: "dict-persist" }` 返却 |

### 10.5 既存 baseline と前提

- **default features**: 175 PASS を維持(P2-A 確定値、`kotoha-storage` 不引込)
- **dict feature**: 223 PASS を維持(P2-A 確定値、UserVocab 無効)
- **dict-persist feature(P2-B 追加)**: §10.1〜10.4 の追加 test 件数(34〜52 件)を上乗せ
- **P2-A hardening 完了前提**: §3.10 のとおり pre-PR で hardening 10 items を投入済を前提とし、P2-B code PR は hardening の上に積む

### 10.6 test 前提条件(dev-dep)

Layer 3 / Layer 4 の test は以下 dev-dependency を P2-A hardening pre-PR(ISSUE #94 P2-B section H8、§3.10)で導入済であることを前提とする。

| dev-dep | 用途 |
|---|---|
| `serial_test` | §10.3.1 の `KOTOHA_DATA_DIR` env var 操作 race 防止(`#[serial]` annotation) |
| `tempfile` | per-test 物理 path 分離(`tempdir().path()`)|
| `assert_cmd` | CLI binary の subprocess 実行 + exit code / stdout / stderr assertion |

## 11. Effort estimate と PR 分割

### 11.1 PR 分割

P2-B は以下 3 PR に分割する。

| PR | 内容 | 見積工数 |
|---|---|---|
| **P2-A hardening pre-PR** | ISSUE #94 P2-B section の P2-A hardening 10 items(`#[non_exhaustive]` 漏れ補完、validation gap、test fixture 整理等) | **0.5 day** |
| **P2-B docs PR** | 本 spec + ADR 0014 D7 + ADR 0015 + Phase 2 spec §3.2 / §5.2 / §6.1 / §8 / §9 改訂 + ROADMAP §P2-B 更新 + glossary.md 5 用語追加 | **0.7 day** |
| **P2-B code PR** | `kotoha-storage` 新 crate + `kotoha-core::dict::user_vocab::UserVocab` + `kotoha-cli::dict_cli` + `kotoha-dict` binary + Layer 1〜4 test | **5.3 day** |
| **合計** | | **6.5 day** |

### 11.2 PR Review Matrix(global CLAUDE.md)適用

| PR | 規模目安 | tier | 適用 review skill |
|---|---|---|---|
| P2-A hardening pre-PR | ≤7 files / ≤200 lines 想定 | Small | `agent-teams:team-review`(security + architecture + testing) + `secrets-check` |
| P2-B docs PR | 6 ファイル変更、約 +900 lines(本 spec 約 700 + ADR 0015 約 200 + 他は小幅追記) | Medium | `agent-teams:team-review`(全 5 dim) + `owasp-security` + `secrets-check` |
| P2-B code PR | 約 15 files / 約 800 lines 想定(Branch Scope Policy 範囲内) | Medium | `agent-teams:team-review`(全 5 dim) + `owasp-security` + `secrets-check` + `database-migrations:sql-migrations`(SQL migration review) + `dependency-audit`(`rusqlite` / `clap` 新規依存) |

### 11.3 ROADMAP 目安(3〜5 day)からの超過理由

ROADMAP §P2-B は工数目安 3〜5 day としていたが、本 spec で 6.5 day に再見積した。超過理由は以下 3 点である。

- **SQLite layer 厚み**: TSV 第一候補から SQLite 共用 DB に方針転換した結果、`kotoha-storage` 新 crate / `Database` / Migration runner / `UserVocabStore` trait + impl + Mock の 4 component を追加実装する必要があり、TSV reader 単体の元見積(~1 day)から大幅に増加
- **validation 同時投入**: ISSUE #94 A03 を P2-B 着地時に同時投入する判断(§3.9)により、validation module + 全経路適用 + test 追加で +0.5 day
- **spec 同期更新**: parent-spec §3.2 / §5.2 / §6.1 / §8 / §9 の 5 箇所を後付け改訂する spec 同期作業で +0.3 day

3 PR 分割により、1 PR あたりは Small(0.5 day) + Medium(0.7 day) + Medium(5.3 day)に収まり、Branch Scope Policy(20 files / 1000 lines / 2 day)の guideline と整合する。code PR の 5.3 day は 2 day cap を超過するが、SQLite + CLI + DictionaryBackend 統合は機能境界で分割するとレビュー文脈が崩れるため単一 PR で提出する(P2-A §3.2 Q2 と同 pattern)。

## 12. 参照

### 12.1 上位 spec / ADR

- 上位 spec(parent-spec): [`docs/specs/_uncategorized/kotoha-phase-2.md`](./2026-04-25-kotoha-phase-2-design.md)
- 兄弟 spec(P2-A、`VocabularyLookup` trait の先出し根拠): [`docs/specs/_uncategorized/p2-a-dictionary-layer.md`](./2026-04-25-p2-a-dictionary-layer-design.md)
- ADR 0014(Phase 2 dictionary layer architecture、本 P2-B docs PR で D7 改訂): [`docs/adr/0014-phase-2-dictionary-layer-architecture.md`](../../adr/0014-phase-2-dictionary-layer-architecture.md)
- ADR 0015(`kotoha-storage` SQLite 採用、本 P2-B docs PR で新規起票): [`docs/adr/0015-kotoha-storage-sqlite-adoption.md`](../../adr/0015-kotoha-storage-sqlite-adoption.md)
- ADR 0011(`#[non_exhaustive]` enum 拡張、`DictionaryConfig` field 追加の根拠): [`docs/adr/0011-kanji-backend-trait-design.md`](../../adr/0011-kanji-backend-trait-design.md)
- ADR 0012(`default = []` feature flag 方針、`dict-persist` feature 命名整合): [`docs/adr/0012-feature-flag-design-for-llama-cpp.md`](../../adr/0012-feature-flag-design-for-llama-cpp.md)

### 12.2 ROADMAP / ISSUE

- ROADMAP §P2-B(本 P2-B docs PR で更新): [`docs/ROADMAP.md`](../../ROADMAP.md)
- ISSUE #95(本 spec の起票 ISSUE)
- ISSUE #94(P2-A hardening 10 items、本 P2-B 着地で A03 同時投入)
- ISSUE #92(Layer 3 deferred、P2-A の 530-case golden runner、本 P2-B とは独立)

### 12.3 関連 memory

- Clean Architecture / SOLID 整合: `~/.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/feedback_clean_architecture_solid.md`
- Vocab 文法的多義性 feedback(§3.7 lookup priority の根拠): `~/.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/feedback_vocab_grammatical_collision.md`
- Phase 1 完了状況: `~/.claude/projects/-home-kohshiro-develops-student-kotoha-ime/memory/project_phase1_completion.md`

### 12.4 外部参照

- rusqlite: <https://github.com/rusqlite/rusqlite>(MIT、`bundled` feature で SQLite C library 同梱)
- SQLite: <https://www.sqlite.org/>(public domain)
- XDG Base Directory Specification: <https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html>
- CVE-2021-42574(Trojan Source、bidi character validation の根拠): <https://nvd.nist.gov/vuln/detail/CVE-2021-42574>
