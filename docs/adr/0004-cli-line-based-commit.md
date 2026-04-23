# 0004: Phase 0 CLI は stdin の改行境界で commit を発火する

## ステータス

承認 (2026-04-24)

## コンテキスト

Phase 0 CLI `kotoha-romaji` は、`InputContext::commit()` を呼び出す契機 (= ユーザが「確定」を意図するタイミング) を何らかの方法で定義する必要がある。IBus engine では物理 Enter キーの `KeyPress` イベントが commit の契機になるが、Phase 0 CLI は標準入力を受け取るだけなので、キーイベントに相当する signal を文字列表現から抽出しなければならない。

spec §10.2 は「標準入力から行単位で読み込み、**行末を Enter (commit) として扱う**」と normative に定めており、本 ADR はこの「改行 = commit」という対応付けを記録する。改行を commit 契機に採用した結果、M6 の `scripts/phase0-smoke.sh` は `printf "line1\nline2\n" | kotoha-romaji` のようなパイプ入力で、commit を挟んだ複数行シーケンスを簡潔に表現できる (spec §13.2)。

## 検討した選択肢

### 選択肢 1: stdin の改行境界で commit する (採用)

- 利点:
  - spec §10.2 の normative 仕様と一致する。
  - シェルの `echo` / `printf` と組み合わせるだけで commit を含むシーケンスを記述でき、smoke test スクリプトが簡潔になる。
  - 「1 行 1 commit」という規則は読み手にとって予測しやすく、`--show-mode` の出力 (1 行 1 モード表示) とも自然に整合する。
- 欠点:
  - 「1 つの preedit を複数の Enter を挟んで構成する」ような複合シナリオは CLI では表現できない。Phase 0 の smoke test 範囲ではこの限界は問題にならない。

### 選択肢 2: 句読点 / 空白境界で commit する

- 利点: セマンティックに「語」単位で確定が走るため、出力が見やすくなる可能性がある。
- 欠点:
  - ローマ字入力文脈で「語」を境界付ける規則を別途定義する必要があり、仕様が肥大化する。
  - 本物の IME (Mozc 等) は commit をユーザ操作 (Enter) で駆動しており、句読点で自動 commit する挙動は既存 IME 慣習と矛盾する。spec §8 の状態機械とも整合しない。

### 選択肢 3: EOF まで一切 commit しない

- 利点: Phase 0 CLI を「streaming input_char の単純なフィルタ」に留められ、実装が最小化される。
- 欠点:
  - commit を伴う挙動 (Transient Direct → Hiragana 自動復帰、direct_buffer の flush) を CLI 経由で検証できず、smoke test の価値が著しく下がる。spec §13.2 の assertion 要件を満たせない。

## 決定

**選択肢 1 (stdin の改行境界で commit する)** を採用する。具体的には:

1. `kotoha-romaji` は stdin を `BufRead::lines()` で 1 行ずつ読み、各行について全文字を `InputContext::input_char` に順次渡したあと `InputContext::commit()` を呼ぶ。
2. 本対応付けは Phase 0 CLI 限定であり、`InputContext::commit` の仕様 (spec §8.4) は変更しない。
3. Phase 3 の IBus engine では本対応付けを使わず、ユーザの Enter `KeyPress` イベントで `commit` を呼ぶ。

## 影響

### Phase 0 への影響

- M6 PR #53 で実装済みの `kotoha-romaji` 本体 (`crates/kotoha-cli/src/bin/kotoha-romaji.rs`) が本 ADR の契機モデルを実装している。本 ADR は当該実装の設計判断を事後的に normative 化する。
- `scripts/phase0-smoke.sh` の assertion (spec §13.2) は全て改行 = commit の前提で書かれており、本 ADR により正当化される。

### Phase 3 以降への影響

- Phase 3 の IBus engine では、ユーザが押した物理 Enter キーイベントを `commit` 契機に使う。Phase 0 CLI の「改行 = commit」対応付けは CLI の外部インタフェース上の制約であり、コアエンジン (`InputContext`) の挙動には影響しない。
- 行途中の明示トグル (spec §10.4 で Phase 0 非対応) を Phase 3 で導入する場合、本 ADR の「改行 = commit」規則は CLI 側のみの制約として維持され、IBus engine 側には持ち込まれない。

## 参照

- ADR 0002 — 本 ADR が前提とする Transient-by-default モード仕様 (commit が Transient → Hiragana 自動復帰の契機)。
- Spec §10.2 — Phase 0 CLI 動作仕様 (「行末を Enter (commit) として扱う」)。
- Spec §13.2 — smoke test アサーション (改行 = commit 前提で記述)。
- Spec §8.4 — `InputContext::commit` 擬似コード。
- M6 PR #53 — `kotoha-romaji` CLI と `scripts/phase0-smoke.sh` の実装。
- ISSUE #49 — M6 tracking ISSUE。
