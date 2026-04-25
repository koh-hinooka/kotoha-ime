# ADR 0015 — `kotoha-storage` crate を新設し SQLite を Phase 2 永続化層として採用する

- **Status**: Accepted (2026-04-25)
- **Date**: 2026-04-25
- **Deciders**: Kotoha Phase 2 maintainers
- **Related ISSUE**: #95 (P2-B design spec docs PR)
- **Related ADR**: ADR 0014 D7(本 ADR と同一 PR で新規追加)、ADR 0011(`#[non_exhaustive]` enum 拡張点)、ADR 0012(feature flag default = [] 方針)

## Context

Kotoha Phase 2 P2-B(User dictionary)着手時点で、UserVocab 永続化形式を確定する必要がある。Phase 2 spec(`docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`)§5.2 では永続化 format の候補を「TOML / JSONL / TSV のいずれか、P2-A で empirical 確定」と stub 化していた。さらに parent-spec §5.2 は Learning cache(P2-C 範囲)についても同様に「TSV 第一候補」と暫定的に記載し、Phase 5 personalization で append 書込みが必要となった時点で JSONL への migration を検討すると論じていた。

しかし、P2-B kick-off brainstorming(2026-04-25)で以下 4 点の前提を再評価した結果、TSV / JSONL / TOML の 3 候補は Phase 2 全体で求められる要件を満たさないことが判明した。

### C1. UserVocab + LearningCache の同時永続化要件

P2-B(UserVocab)と P2-C(LearningCache)は同一プロセス内で同時に永続化される 2 種類の語彙ストアである。両者は (a) 共通の起動 / shutdown lifecycle、(b) 共通のバックアップ / 削除 / 移行運用、(c) UserVocab で登録した surface が LearningCache でも reinforce されるユースケース、の 3 観点で密結合する。TSV / JSONL を採用すると 2 file 並立になり、ATOMIC transaction を 2 file 跨ぎで取得できない。

### C2. concurrent / crash safety

`kotoha-dict` CLI と Phase 3 IBus engine は同一 DB を別プロセスから同時に open するシナリオが発生する。TSV / JSONL は OS file lock(`flock` / `fcntl`)で排他制御を実装する必要があり、crash 時の partial write 復旧経路を独自実装する必要がある。SQLite の WAL モードはこれらをアプリ実装なしで提供する。

### C3. schema 進化(Phase 5 personalization)

Phase 5 custom model の personalization(ADR 0010 / Phase 5 spec §3.5)では、LearningCache レコードに `context_embedding`(数百次元 vector)等の追加 field を導入する可能性がある。TSV / JSONL に追加 field を投入する場合、column 順序 / null 表現 / version field 等の設計を独自に行う必要があり、誤読み込みの risk が伴う。SQLite は `ALTER TABLE ADD COLUMN` で正式な migration を提供する。

### C4. escape / edge case

UserVocab の surface / reading は日本語文字列を含み、TAB / 改行 / NULL 文字 / bidi character / 制御文字の混入を validation で reject する必要がある。TSV は TAB / 改行を escape する公式仕様がなく、JSONL も Unicode escape の実装差異が runtime バグを生む。SQLite はバイナリセーフな TEXT 型で escape 不要。

## Decision

本 ADR では以下 2 項目を決定する。

### D1. SQLite(`rusqlite + bundled`)を Phase 2 永続化層として採用する

Kotoha Phase 2 は UserVocab(P2-B)と LearningCache(P2-C)の永続化層に SQLite を採用する。SQLite library は `rusqlite`(`bundled` feature 有効)を選択する。

採用 schema は単一 DB ファイル `kotoha.db` 内に `user_vocab` table と `learning_cache` table を同梱し、`PRAGMA user_version` + `include_str!` で同梱した `migrations/v001_initial.sql` を起動時 apply する。詳細は P2-B spec §5 を参照。

### D2. 新 crate `kotoha-storage` を導入し永続化層を独立 crate として分離する

`rusqlite` の C 依存(SQLite C library)を `kotoha-core` から隔離するため、新 crate `crates/kotoha-storage/` を導入する。`kotoha-core::dict::user_vocab::UserVocab` は `kotoha-storage::user_vocab::UserVocabStore` trait を `Box<dyn UserVocabStore>` で field 保持し、Clean Architecture「Interface 依存」/ SOLID DIP に整合させる。

依存方向は以下のとおりである。

```text
kotoha-cli (binary: kotoha-dict)  ───┐
                                     ├──► kotoha-core ──► kotoha-storage
                                     │       │              │
                                     │       │ (trait)      │ (impl)
                                     │       ▼              ▼
                                     │   UserVocabStore     SqliteUserVocabStore
                                     │
                                     └──► kotoha-storage (直接 CRUD: kotoha-dict 専用)
```

`kotoha-storage` は `kotoha-core` に逆依存しない。

## 比較した候補

P2-B kick-off brainstorming で永続化 format / RDBMS 候補 15 案を比較した。早期却下 9 案と finalist 6 案に分け、finalist は 6 軸の比較表で評価した。

### 早期却下した 9 案

以下 9 案は単一観点で Kotoha Phase 2 要件と矛盾するため、finalist から除外した。

| 候補 | 早期却下理由 |
|---|---|
| YAML | 仕様の曖昧性(YAML 1.1 / 1.2 / Norway problem 等)、Rust crate ecosystem の不安定性、binary safety なし |
| MessagePack | binary format で human-readable でなく、debug 用 `sqlite3` 同等の inspection tool が乏しい |
| CBOR | MessagePack と同じく binary format、IME runtime に持ち込む利点が薄い |
| Parquet | columnar format で OLTP 用途に不適、small write が高コスト |
| Protobuf | schema 定義と `.proto` からの code generation step が必要、Kotoha の単純 CRUD に過剰 |
| Cap'n Proto | schema 定義と code generation step が必要、Rust ecosystem の成熟度が SQLite に劣る |
| FlatBuffers | schema 定義と code generation step が必要、IME OLTP 用途に対し zero-copy の利点が薄い |
| RON(Rust Object Notation) | Rust 専用 format、外部 inspection tool / 他言語 binding なし |
| 独自 binary format | 実装コストと crash recovery の独自設計が正当化できない |

### Finalist 6 案の比較表

以下 6 案を 6 軸で比較した。各セルは `+`(優位)/ `=`(中庸)/ `-`(劣位)で評価する。

| 候補 | schema 進化 | concurrent 安全 | crash 安全 | escape edge case | Phase 5 personalization 適合 | 工数(初期実装) |
|---|---|---|---|---|---|---|
| TSV | -(version field 自前) | -(flock 自前) | -(partial write 復旧自前) | -(TAB / 改行 escape 仕様なし) | -(field 追加で全行再 parse) | +(parse 容易) |
| CSV | -(同上) | - | - | =(RFC 4180 準拠ライブラリあり) | - | + |
| JSONL | =(version field 自前) | - | - | =(Unicode escape 実装差異あり) | =(field 追加は append 容易) | + |
| TOML | -(配列形式に難) | - | - | +(ライブラリ成熟) | -(file 全体 rewrite が前提) | =(table 配列の表現が冗長) |
| JSON(単一 file) | =(version field 自前) | - | -(部分書き戻し不可) | + | - | = |
| **SQLite** | **+(`PRAGMA user_version` + `ALTER TABLE`)** | **+(WAL モード)** | **+(WAL + journal)** | **+(TEXT バイナリセーフ)** | **+(field 追加が migration で完結)** | **=(rusqlite + bundled)** |

SQLite は schema 進化 / concurrent 安全 / crash 安全 / escape edge case / Phase 5 personalization 適合の 5 軸で `+` を獲得し、工数軸でも `=`(`rusqlite + bundled` の依存追加と migration 同梱で実装可能)に留まる。他 5 候補は最低でも 3 軸が `-` であり、Phase 2 全体の要件を満たさない。

## Decision rationale

### R1. ACID + WAL によるアプリレベル lock 不要

SQLite の WAL モード(`PRAGMA journal_mode = WAL`)は writer 1 / readers N の並行性を OS-level lock のみで提供し、アプリ層での `flock` / `fcntl` 実装が不要になる。Phase 3 IBus engine と `kotoha-dict` CLI が同一 DB を別プロセスから open するシナリオでも、SQLite が writer ロックを管理する。

### R2. `PRAGMA user_version` で正式 versioned migration

`PRAGMA user_version`(SQLite が table 外で保持する 32-bit 整数)を migration version として使用し、`include_str!("migrations/v001_initial.sql")` で SQL をバイナリ同梱する。distribution 時に migrations ディレクトリを別配布する必要がない。

### R3. Phase 2 で UserVocab + LearningCache を同 DB に集約、ATOMIC transaction 跨ぎ

`user_vocab` table と `learning_cache` table を `kotoha.db` 単一 DB 内に配置することで、`BEGIN TRANSACTION` を 2 table 跨ぎで取得可能となる。Phase 5 で UserVocab 登録と LearningCache 強化を同時に行う user 操作(例: `kotoha-dict add` 直後に user が同 entry を確定して LearningCache を更新する)が ATOMIC に成立する。

### R4. Phase 5 personalization での field 追加が schema migration で完結

Phase 5 custom model の personalization で `learning_cache` に `context_embedding`(BLOB or 拡張 TEXT) field を追加する場合、`ALTER TABLE learning_cache ADD COLUMN context_embedding BLOB` の 1 文で migration が完結する。TSV / JSONL では全 row の再 parse + rewrite が必要となる。

### R5. Desktop runtime 採用実績

SQLite は desktop application runtime として広範な採用実績を持つ。例として以下が挙げられる。

- Firefox `places.sqlite`(履歴 / bookmark)
- Chrome `Cookies` / `History` / `Login Data`(Cookie / 履歴 / パスワード)
- iOS `Photos.sqlite`(写真メタデータ)
- Apple Mail / Apple Calendar(macOS 内部ストレージ)
- Notion / Obsidian / VS Code(設定 / index)

これらの runtime 規模は Kotoha(UserVocab 数千件 + LearningCache LRU 上限 10,000)を遥かに超える。Kotoha の規模は SQLite の適用域の極めて下端であり、性能 / footprint 上の懸念はない。

### R6. PostgreSQL / Docker 不採用根拠

P2-B brainstorming Q2 で「Docker 同梱の PostgreSQL を採用する案」も検討したが、以下 4 点で却下した。

- **Docker daemon 起動が IME runtime の前提条件になる**: kotoha-ime は GNOME session 開始時に IBus engine として spawn される。Docker daemon が systemd で起動済でない distro / 環境では IME 自体が起動しない。distro 配布難度が爆発する
- **desktop standalone IME 方針との衝突**: ADR 0010 / Phase 5 spec は「desktop で standalone 動作する IME」を明示方針としており、external service(PostgreSQL container)を起動前提とする設計と整合しない
- **container resource footprint**: PostgreSQL container は最小構成でも数百 MB の RAM / disk を常駐 footprint として要求する。Kotoha の常駐 footprint(Phase 1 baseline で約 2.0 GB、ADR 0014 Consequences)に上乗せする許容圏外
- **Phase 3 IBus engine との lifecycle 不整合**: Phase 3 IBus engine は GNOME Mutter から spawn / kill される lifecycle を持つ。PostgreSQL container は GNOME session とは独立な systemd service として常駐する必要があり、IME プロセスの spawn / kill のたびに connection pool の再確立が発生する

なお、SudachiDict-core の 76 万 entries(`system_core.dic`)は別レイヤー(`sudachi.rs` の binary format `.dic`)で扱われており、UserVocab + LearningCache の規模(数千件 + 上限 10,000)とは桁違いに小さい。「76 万 entries 規模だから RDBMS が必要」との誤前提は成立しない。

## Consequences

### 正の帰結

- **新 crate `kotoha-storage` 追加で C 依存を 1 crate に閉じ込め**: `rusqlite + bundled` の SQLite C library コンパイルは `kotoha-storage` build 時のみ発生し、`kotoha-core` 単体 build / Layer 1 unit test / `MockBackend` 経路は C コンパイル不要となる。Phase 1 baseline の build 速度が退行しない
- **Phase 2 spec §5.2 の TSV 第一候補が archived として明確化**: parent-spec §5.2 の旧 TSV / JSONL / TOML 比較は本 ADR で archived となり、Phase 2 全体の persistence 方針が SQLite に統一される
- **Phase 5 personalization への migration path が確保される**: schema 進化 / field 追加 / cross-table reference が `ALTER TABLE` + migration 番号上書きで完結し、Phase 5 design 着手時の persistence layer 再設計コストが発生しない
- **WAL モードによる concurrent open の構造的安全性**: Phase 3 IBus engine と `kotoha-dict` CLI の同時 open シナリオが SQLite の WAL モード機能で覆われ、独自 file lock 実装が不要

### 正の帰結(続き、Phase 5 trust boundary)

- **Phase 5 trust boundary に対する prompt-injection 耐性**: Phase 5 KotohaNative model(ADR 0010)では、UserVocab の `surface` / `reading` / `pos` 内容が prompt 構築経路に流れ、custom model の context として model 入力に到達する。本 ADR の design では P2-B spec §9.1 で `surface` / `reading` / `pos` 全 field に対し PUA(U+E000..=U+F8FF + U+F0000..=U+10FFFD)/ Variation Selector(U+FE00..=U+FE0F)/ Tag character(U+E0000..=U+E007F)の 3 char class を reject し、Phase 5 Mixed JP/EN allowlist (U+EE00..=U+EE03) のみを Karukan 互換の特例として許容する。これにより visual に invisible / 無害に見える PUA tokens を悪用する prompt-injection 経路を Phase 2 段階で先回り遮断する

### 負の帰結

- **human-readable file ではなくなる**: TSV / JSONL のように `cat` / `less` で内容を直視できず、`sqlite3` CLI(distro `sqlite3` package 経由)で inspect する運用が必要になる。debugging / backup の手順を README に追記する必要がある
- **`rusqlite + bundled` の追加依存**: SQLite C library のコンパイルが初回 `cargo build` で発生する(数十秒〜1 分程度)。Phase 1 の `llama-cpp-2` C++ build chain(初回 2〜5 分、incremental 30 秒)と比較すれば軽量だが、依存追加自体は ADR 0012 の minimal dependency 方針に対する偏差として記録する
- **schema migration は forward only**: down migration / rollback は実装しない方針(P2-B spec §3.6)。recover 経路は「DB 削除 + 再起動で空 DB 再作成」となり、UserVocab / LearningCache の content が消失する

## Alternatives considered

### A1. TSV(parent-spec §5.2 の旧第一候補)

**Rejected.** schema 進化(field 追加で全行再 parse)、concurrent 安全(`flock` 自前実装)、crash 安全(partial write 復旧自前)、escape edge case(TAB / 改行の escape 仕様なし)の 4 軸で SQLite に劣る。Phase 5 personalization で `context_embedding` field を追加する場合に migration コストが直接 user 体験に跳ねる(起動時の全行再 parse による latency 退行)。

### A2. JSONL

**Rejected.** TSV と同じ 4 軸で劣る。JSON Unicode escape の実装差異が runtime バグを生む可能性があり(例: surrogate pair の escape 表現が serde_json と他言語 parser で異なる)、Phase 5 で外部 tool が JSONL を読む可能性のあるシナリオで integrity risk が増す。

### A3. TOML

**Rejected.** TOML は configuration file format として最適化されており、レコード列(table 配列)の表現が冗長で large file 性能が悪い。Phase 2 LearningCache の上限 10,000 entry 規模で起動時 parse cost が許容圏を超える可能性がある(empirical 未測定だが、TOML crate の large file 性能は SQLite の indexed read に明確に劣る)。

### A4. PostgreSQL(Docker container 同梱)

**Rejected.** `Decision rationale` R6 で詳述した 4 点(Docker daemon 前提、desktop standalone IME 方針衝突、container resource footprint、IBus lifecycle 不整合)で desktop IME runtime に不適合。

### A5. embedded RDBMS の他候補(DuckDB / sled / RocksDB)

**Rejected.**

- **DuckDB**: OLAP(analytical query)向けに最適化されており、Kotoha の OLTP(point lookup + small write)用途に過剰。binary size も SQLite より大きい
- **sled**: Rust native KV store だが β 版扱いであり、production runtime としての安定性が SQLite に劣る
- **RocksDB**: KV store で Kotoha の table-like data model に対し abstraction が低すぎる、C++ 依存で binary size が大きい(数十 MB)

### A6. 独自 binary format

**Rejected.** crash recovery / WAL / migration の独自実装が正当化できる規模・性能要件は Kotoha Phase 2 にはない。SQLite を使う方が実装行数が大幅に少なく済む。

## Related documents

- 上位 ADR: ADR 0014(Phase 2 dictionary layer architecture、本 ADR と同一 PR で D7 節を新規追加)
- 上位 spec: Phase 2 spec(`docs/superpowers/specs/2026-04-25-kotoha-phase-2-design.md`)§5.2 の persistence は本 ADR で archived
- 子 spec: P2-B spec(`docs/superpowers/specs/2026-04-25-p2-b-user-dictionary-design.md`)§3 / §5 / §6 が本 ADR の実装詳細を担う
- 関連 ADR: ADR 0011(`#[non_exhaustive]` enum 拡張、`DictionaryConfig` field 追加の根拠)、ADR 0012(`default = []` feature flag 方針、`dict-persist` feature 整合)
- 関連 ISSUE: #95(P2-B docs PR の起票 ISSUE)、#94(P2-A hardening 10 items + A03 validation 同時投入)
- 参考実装: Firefox places.sqlite / Chrome cookies / iOS Photos.sqlite(R5 desktop runtime 採用実績)
- 関連 brainstorming: 2026-04-25 P2-B kick-off brainstorming(本 ADR の判断根拠)

## Note: 本 ADR の status について

本 ADR は P2-B docs PR 投入時点で Accepted とする。P2-B code PR で `kotoha-storage` crate と `migrations/v001_initial.sql` を実装し、Phase 2 closure(P2-D 完了)時点で本 ADR に「実装後の empirical 確認」節を追記する予定である。Phase 5 personalization で schema 進化が必要となった時点で、別 ADR で v002 schema を起票する。
