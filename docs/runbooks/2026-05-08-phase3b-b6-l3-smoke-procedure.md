# Phase 3-B B6-c: L3 manual smoke procedure (GNOME Wayland)

| 項目 | 値 |
|------|----|
| ISSUE | [#196](https://github.com/std-koh-hinooka/kotoha-ime/issues/196) (parent: [#136](https://github.com/std-koh-hinooka/kotoha-ime/issues/136)) |
| 関連 spec | `docs/specs/_uncategorized/p3-a-ibus-engine.md` §10 / §11、`docs/specs/_uncategorized/p3-b-ibus-listener.md` |
| 関連 ADR | ADR 0020(event-loop architecture)、ADR 0021(zbus integration) |
| 前提 PR | #183 (B0h-f + B3 event-loop)、#194 (#187 ShutdownObserver)、#199 (#195 zbus listener decode)、#198 (#197 SIGTERM hook) |
| 想定実施日 | 2026-05-08 以降 |

## 目次

1. [§1 目的と完了条件](#1-目的と完了条件)
2. [§2 前提環境](#2-前提環境)
3. [§3 セットアップ](#3-セットアップ)
4. [§4 IBus daemon への engine 登録](#4-ibus-daemon-への-engine-登録)
5. [§5 テストマトリクス(3 app × 10 シナリオ)](#5-テストマトリクス3-app--10-シナリオ)
6. [§6 シナリオ詳細](#6-シナリオ詳細)
7. [§7 アプリ別補足手順](#7-アプリ別補足手順)
8. [§8 結果記録テンプレート](#8-結果記録テンプレート)
9. [§9 障害発生時のエスカレーション](#9-障害発生時のエスカレーション)
10. [§10 クリーンアップ](#10-クリーンアップ)
11. [§11 完了判定とフォローアップ](#11-完了判定とフォローアップ)

---

## §1 目的と完了条件

### 目的

Phase 3-B 残作業 **B6-c**(spec §10.1 row L3)を完了する。Phase 3-A spec §11 受け入れ基準のうち、自動テストでは検証不可能な以下を実機で確認する:

- 実機(GNOME Wayland)で `cargo run --bin kotoha` 起動が IBus daemon に登録される
- 典型 10 件の変換シナリオが Firefox / GNOME Text Editor / VS Code 全てで動作する
- coalescing window 値(`LIVE_WINDOW = 7ms` / `COMMIT_WINDOW = 30ms`)が体感上適切

### 完了条件(本手順書 PASS で v0.3.0 milestone close 可)

- [ ] 全 30 セル(3 app × 10 シナリオ)が PASS、または FAIL は別 ISSUE 化済
- [ ] WBS 結果ログ(§8 テンプレート)を `docs/runbooks/<yyyy-MM-dd>-phase3b-b6-l3-smoke-result.md` として commit
- [ ] coalescing window 体感観察を §11 でメモ(変更要なら ADR 追補)
- [ ] ROADMAP の #136 を `[x]` 化、Phase 3 milestone v0.3.0 を「Active」→「完了済」へ移動
- [ ] annotated git tag `v0.3.0` + リリース ADR 起票(`docs/adr/NNNN-release-v0.3.0.md`)

---

## §2 前提環境

| 項目 | 要件 |
|------|------|
| OS | Linux(GNOME 上で動作するディストリビューション) |
| Display protocol | **Wayland**(`echo $XDG_SESSION_TYPE` で `wayland` 確認) |
| GNOME version | 45.x 以降推奨(Mutter は IBus を IM framework として使用) |
| IBus | 1.5.x 以降(`ibus version` で確認) |
| dbus session bus | active(`echo $DBUS_SESSION_BUS_ADDRESS` で値が表示される) |
| Rust toolchain | 1.80 以降(`rustc --version`) |
| 対象アプリ | Firefox 最新、GNOME Text Editor(`gnome-text-editor`)最新、VS Code 最新 |

### 環境確認コマンド

```bash
echo "Display: $XDG_SESSION_TYPE"  # 期待: wayland
ibus version                       # 期待: IBus 1.5.x or later
which gnome-text-editor            # 期待: /usr/bin/gnome-text-editor
which firefox                      # 期待: /usr/bin/firefox or snap path
which code                         # 期待: /usr/bin/code or snap path
rustc --version                    # 期待: 1.80.0 or later
```

いずれかが満たされない場合は本手順を中断し、§9 のエスカレーション手順に従う。

---

## §3 セットアップ

### §3.1 リポジトリ最新化

```bash
cd ~/develops/student/kotoha-ime
git checkout develop
git pull --ff-only
git log --oneline -3
# 期待: HEAD は #199 (zbus listener decode) merge を含む
# 例: c9fd60a chore(deps,docs)... / 50b3b7c feat(engine-ibus,bin): production IBus listener... (#195)
```

### §3.2 release build(production binary)

開発時の `cargo run` は debug build で latency が production と異なるため、smoke は **release build** で実施する。

```bash
cargo build --release --bin kotoha
ls -la target/release/kotoha
# 期待: 数 MB ~ 数十 MB の実行ファイルが存在
```

### §3.3 stub symbol が release binary に含まれないことの再確認

PR #160(B0h-e、ISSUE #159)で確立された invariant:

```bash
nm target/release/kotoha 2>/dev/null | rg -i 'stub' | head -5 || echo "no stub symbols (expected)"
# 期待: "no stub symbols (expected)" のみ表示
```

stub symbol が出る場合は build feature flag が誤っている可能性あり。`cargo build --release --bin kotoha`(default features)で再 build。

### §3.4 リソースディレクトリの確認

| リソース | 配置 | 用途 |
|---|---|---|
| SudachiDict-core (`system_core.dic`) | `~/.config/kotoha/sudachi-dict/` または環境変数 | Phase 2-A 辞書 lookup |
| Gemma GGUF model | 未配置の場合 stub fallback(env で許可)| Phase 1 LLM backend |
| Kotoha SQLite store | `~/.config/kotoha/kotoha.db`(初回起動で自動生成) | Phase 2-B/C user dict + learning cache |

詳細は `crates/kotoha-bin/src/main.rs` の DI 段(step 1-9)を参照。

---

## §4 IBus daemon への engine 登録

### §4.1 component 記述ファイルの一時配置

本 PR scope 外(packaging task で正規化予定、§11 参照)のため、開発時は手動配置で daemon に engine を認識させる。

`~/.config/ibus/component/kotoha.xml` を以下の内容で作成:

```xml
<?xml version="1.0" encoding="utf-8"?>
<component>
  <name>org.freedesktop.IBus.Engine.Kotoha</name>
  <description>Kotoha IME (Phase 3-B smoke build)</description>
  <exec>__KOTOHA_BIN_PATH__</exec>
  <version>0.3.0-pre</version>
  <author>std-koh-hinooka</author>
  <license>MIT OR Apache-2.0</license>
  <homepage>https://github.com/std-koh-hinooka/kotoha-ime</homepage>
  <textdomain>kotoha</textdomain>
  <engines>
    <engine>
      <name>kotoha</name>
      <language>ja</language>
      <license>MIT OR Apache-2.0</license>
      <author>std-koh-hinooka</author>
      <icon></icon>
      <layout>default</layout>
      <longname>Kotoha</longname>
      <description>Kotoha Japanese IME</description>
      <rank>0</rank>
    </engine>
  </engines>
</component>
```

`__KOTOHA_BIN_PATH__` は §3.2 の release binary 絶対パスに置換(例:`/home/kohshiro/develops/student/kotoha-ime/target/release/kotoha`)。

### §4.2 ibus daemon の再読み込み

```bash
ibus restart
sleep 2
ibus list-engine | rg -i kotoha
# 期待: kotoha - Kotoha 等の行が表示される
```

`kotoha` が表示されない場合:

- `~/.config/ibus/component/kotoha.xml` の path / xml syntax を確認
- `journalctl --user -u ibus.service -n 50` で daemon ログ確認
- `<exec>` パスが実際の binary を指していて実行権限があるか確認(`ls -la <path>`)

### §4.3 input source への kotoha 追加

GNOME Settings → Keyboard → Input Sources →「+」→「Japanese」→「Kotoha」を追加。

または CLI:

```bash
gsettings set org.gnome.desktop.input-sources sources \
  "[('xkb', 'us'), ('ibus', 'kotoha')]"
```

### §4.4 engine 切替の確認

`Super` + `Space`(または GNOME 設定で割り当てたキー)で input source を Kotoha に切替。
画面右上 / panel に Kotoha 表示が出ることを確認。

---

## §5 テストマトリクス(3 app × 10 シナリオ)

各セルは PASS / FAIL / SKIP のいずれかで結果記録。

| # | シナリオ | Firefox | GNOME Text Editor | VS Code |
|---|---|:---:|:---:|:---:|
| 1 | 純粋ひらがな入力(`ことば` → 「ことば」) | | | |
| 2 | 単漢字変換(`にほん` → 「日本」) | | | |
| 3 | 複合漢字変換(`にほんご` → 「日本語」) | | | |
| 4 | User dict entry 反映(事前登録した語が候補に出る) | | | |
| 5 | Live conversion preedit 表示(打鍵中に候補プレビュー) | | | |
| 6 | Backspace で preedit 縮小 | | | |
| 7 | Enter で commit + preedit クリア | | | |
| 8 | Esc で preedit キャンセル | | | |
| 9 | 変換中 Tab で別フィールドへ focus 移動 → focus_out 反応 | | | |
| 10 | 長文(50 文字+ で 3-4 commit cycle) | | | |

凡例: ✅ PASS / ❌ FAIL / ⏭️ SKIP(理由必須)

---

## §6 シナリオ詳細

各シナリオは「入力」「期待挙動」「観察ポイント」「failure pattern と対処」を含む。

### S1: 純粋ひらがな入力

- **入力**: `kotoba` をローマ字打鍵(k → o → t → o → b → a)
- **期待挙動**:
  - 各打鍵で preedit が `k` → `こ` → `こt` → `こと` → `ことb` → `ことば` のように変化
  - Live conversion 候補が打鍵間で表示される(7ms coalescing 後)
  - Enter で `ことば` が app に commit される
- **観察ポイント**:
  - preedit 文字が描画される latency(< 50ms 体感)
  - Live 候補ウィンドウのちらつき / 残留がないこと
- **failure**: preedit が表示されない → IBus daemon との signal emit が機能していない可能性。`journalctl --user -u ibus.service -n 100` で daemon log を確認

### S2: 単漢字変換

- **入力**: `nihon` 打鍵(`にほん` 完成後 Space 押下)
- **期待挙動**:
  - Space で commit-mode に遷移、候補ウィンドウに `日本 / にほん / 二本 / ...` が表示
  - 1 番目「日本」がデフォルト選択
  - Enter で「日本」commit
- **観察ポイント**:
  - 候補ウィンドウ表示 latency(< 100ms)
  - ranker からの結果到着順序(SudachiDict / UserVocab / LearningCache / LLM 順、§spec 7.3 coalescing window 30ms 内に primary set 揃うこと)

### S3: 複合漢字変換

- **入力**: `nihongo` 打鍵 + Space
- **期待挙動**:
  - 候補に「日本語」が含まれる(SudachiDict A unit / B unit)
  - 1 番目に「日本語」がデフォルト選択(中粒度トークン)
- **failure**: 「日本」「語」が分離して候補化される場合 → SudachiDict prefix lookup の split mode が異なる可能性。spec §6.4 入口の token boundary 仕様を再確認

### S4: User dict entry

- **準備**(§3.2 完了後の事前作業):
  ```bash
  cargo run --release --bin kotoha-dict -- add --kana "ことは" --kanji "琴葉"
  cargo run --release --bin kotoha-dict -- list | rg "琴葉"
  ```
- **入力**: `kotoha` 打鍵 + Space
- **期待挙動**:
  - 候補に「琴葉」が表示される(UserVocab lookup の優先度を SudachiDict より上に置く設計)
- **failure**: 「琴葉」が出ない → kotoha-storage の SQLite ファイルパスを engine 側が正しく開けているか確認(`crates/kotoha-bin/src/main.rs` step 6 storage open)

### S5: Live conversion preedit 表示

- **入力**: `nihongowotsukau`(にほんごをつかう)を 100ms 間隔程度で打鍵
- **期待挙動**:
  - 各打鍵後 7ms 程度で preedit 候補が更新される(`LIVE_WINDOW`)
  - 連続打鍵中も preedit が flicker しない(候補が一瞬空になって再表示等が起きない)
- **観察ポイント**: 連打時に `apply_buffer_update` の Empty Replace 経路に落ちないこと(spec §9.3 silent failure 防止規約)

### S6: Backspace で preedit 縮小

- **入力**: `nihongo` 打鍵後 Backspace 3 回
- **期待挙動**:
  - preedit が `にほんご` → `にほん` → `にほ` → `に` のように 1 文字ずつ削除
  - 候補ウィンドウは preedit 内容に追随して更新
- **failure**: Backspace で preedit が突然空になる → engine 状態機械の `process_key_event` の Backspace 経路 bug。spec §5.2 row 4 を再確認

### S7: Enter で commit

- **入力**: `kotoba` + Space で「言葉」表示 + Enter
- **期待挙動**:
  - 「言葉」が app の現在のフィールドに挿入される
  - preedit が空になる
  - 学習キャッシュに記録される(同一 `kana_input = "ことば"` で次回 lookup 時に頻度が上がっている)
- **学習確認**(任意):
  ```bash
  sqlite3 ~/.config/kotoha/kotoha.db \
    "SELECT kana_input, chosen_kanji, frequency FROM learning_cache WHERE kana_input = 'ことば';"
  ```

### S8: Esc で cancel

- **入力**: `kotoba` 打鍵中(まだ Enter 押下前)に Esc
- **期待挙動**:
  - preedit が空になる
  - 候補ウィンドウが hide
  - app には何も commit されない
  - engine state が `Idle` に戻る

### S9: focus_out 反応

- **入力**: 入力フィールド A に `nihon` を入力中(preedit 表示状態)→ Tab で入力フィールド B へ移動
- **期待挙動**:
  - フィールド A の preedit が消える(自動 commit はしない)
  - 候補ウィンドウが hide
  - フィールド B にカーソル移動
- **観察ポイント**: `Event::IBusReset(IBusResetKind::FocusOut)` が listener から engine_loop に届き engine.focus_out() が呼ばれること(`KOTOHA_LOG=debug` 起動時に `tracing::debug!` で観測可能)

### S10: 長文 commit cycle

- **入力**:`konnichiwakotohajapanesetypingsystemwotoshitemashita` 等、50 文字程度の文を 3-4 文節区切りで Space + Enter を繰り返し commit
- **期待挙動**:
  - 各 commit で preedit が clear、commit_history が蓄積
  - メモリリーク的挙動なし(long run でも応答性 stable)
- **観察ポイント**:
  - 数十回の commit cycle 後も Live latency に劣化なし
  - `htop` 等で kotoha process メモリ使用量が安定(数 MB ~ 数十 MB)

---

## §7 アプリ別補足手順

### §7.1 Firefox

- アドレスバー / 検索 box / Gmail compose / wiki 編集 box などで実施
- 一部 Web text input(contenteditable / iframe 内)では IBus 動作が不安定な事例あり。GNOME Text Editor で先に動作確認後 Firefox へ
- DevTools console で `document.activeElement.tagName` で IME 入力先のタイプ確認可能

### §7.2 GNOME Text Editor

- `gnome-text-editor` で新規空ファイルを開いて実施
- 最も IBus 標準動作が期待できる app(GTK 4 native)。動作の baseline として活用
- フォント設定が日本語に対応していない場合は表示が崩れる(機能テストとは別問題、フォント切替で確認)

### §7.3 VS Code

- VS Code は Electron + Monaco editor で実装。IBus との互換性は version 依存:
  - VS Code 1.95+(Electron 32+):IBus 1.5 対応改善
  - 古い version では preedit 表示遅延 / 文字化けの既知 issue あり(本手順では最新版を前提)
- `~/.config/Code/User/settings.json` で `"editor.inputMode": "input"` 等の関連設定なし(default で IBus)

---

## §8 結果記録テンプレート

`docs/runbooks/<yyyy-MM-dd>-phase3b-b6-l3-smoke-result.md` として作成:

```markdown
---
feature: phase3b-b6-l3-smoke-result
status: implemented
bounded_context: _uncategorized
related_issues: ["#136", "#196"]
related_prs: []
glossary_refs: ["coalescing-window", "event-loop", "ime-engine", "preedit"]
last_reviewed: <yyyy-MM-dd>
---

# Phase 3-B B6-c L3 Manual Smoke Result (<yyyy-MM-dd>)

## §1 環境

| 項目 | 値 |
|---|---|
| Date | <yyyy-MM-dd HH:MM JST> |
| OS | <Ubuntu 24.04 / Fedora 40 / etc> |
| Display | <`wayland` from $XDG_SESSION_TYPE> |
| GNOME | <Settings → About → version> |
| IBus | <`ibus version`> |
| Kotoha rev | <`git rev-parse HEAD`> |
| build profile | release |

## §2 結果マトリクス

| # | シナリオ | Firefox | GNOME Text Editor | VS Code | 備考 |
|---|---|:---:|:---:|:---:|---|
| 1 | 純粋ひらがな入力 | ✅ / ❌ / ⏭️ | ✅ / ❌ / ⏭️ | ✅ / ❌ / ⏭️ | (FAIL の場合理由) |
| 2 | 単漢字変換 | | | | |
| 3 | 複合漢字変換 | | | | |
| 4 | User dict entry | | | | |
| 5 | Live conversion preedit | | | | |
| 6 | Backspace preedit 縮小 | | | | |
| 7 | Enter commit | | | | |
| 8 | Esc cancel | | | | |
| 9 | focus_out (Tab) | | | | |
| 10 | 長文 commit cycle | | | | |

集計: ✅ <count> / ❌ <count> / ⏭️ <count>(全 30 セル)

## §3 観察された latency / coalescing 体感

- preedit 表示 latency(打鍵 → 表示):約 <X> ms 体感
- 候補ウィンドウ表示 latency(Space → 候補リスト):約 <Y> ms 体感
- Live conversion 中の preedit flicker:なし / あり(条件: <...>)
- LIVE_WINDOW = 7ms / COMMIT_WINDOW = 30ms の妥当性:適切 / 引き上げ推奨 / 引き下げ推奨

## §4 FAIL 詳細(あれば)

### S<番号>: <app> でのシナリオ <番号> FAIL

- **再現手順**: <...>
- **期待結果**: <...>
- **実観察結果**: <...>
- **screenshot**: `docs/images/2026-MM-DD-l3-smoke-S<番号>-fail.png`(任意)
- **journalctl 抜粋**: <ibus daemon log の関連行>
- **対応**: 別 ISSUE #XXX 起票 / patch 即時 / wontfix / 別 milestone へ deferral

## §5 SKIP 詳細(あれば)

- S<番号> for <app>: <SKIP 理由(例: app バージョン制約 / 環境差異)>

## §6 結論

- 完了条件達成: 全 30 セル PASS / FAIL は別 ISSUE 化済 / 一部 SKIP は妥当な理由付きで受容
- v0.3.0 milestone close 可否: ✅ 可 / ❌ 不可(理由: <...>)

## §7 follow-up

- coalescing window value 調整必要: なし / あり(ADR 追補 #XXX)
- 別 ISSUE 化した item: <list>
```

---

## §9 障害発生時のエスカレーション

| 障害種別 | 一次対応 | エスカレーション先 |
|---|---|---|
| `kotoha` binary が起動しない | `RUST_BACKTRACE=1 KOTOHA_LOG=debug ./target/release/kotoha 2>&1 | head -50` でログ確認 | エラーが ranker / storage 由来なら別 ISSUE。zbus 由来なら ADR 0021 再評価 |
| ibus daemon に engine が登録されない | `ibus list-engine`、`~/.config/ibus/component/kotoha.xml` syntax 確認、`ibus restart`、`journalctl --user -u ibus.service -n 100` | engine factory registration の packaging task として別 ISSUE |
| preedit が表示されない | IBus 側 component 登録の path 不正 / engine binary が ibus に応答していない | listener 経路の zbus dispatch ログ確認(`tracing::debug!` 経由)、必要なら別 ISSUE |
| 特定 app でのみ FAIL | 他 2 app で PASS なら app 固有 issue として SKIP 記録、別 ISSUE 化 | アプリ側の IBus 互換性。Kotoha 修正対象外 |
| Live conversion が遅い | `KOTOHA_LOG=debug` で worker thread の timing 観測 | LIVE_WINDOW / COMMIT_WINDOW 値の調整可否を ADR 追補で判断 |
| Process が予期せず exit | `journalctl --user -n 200`、core dump 設定確認 | spec §9.3 silent failure 違反。即 ISSUE 起票 |

---

## §10 クリーンアップ

smoke 完了後に環境を元に戻す手順:

```bash
# 1. ibus input source から kotoha を外す
gsettings set org.gnome.desktop.input-sources sources "[('xkb', 'us')]"

# 2. component xml を削除(残置でも害なし、daemon 起動時のみ評価)
rm ~/.config/ibus/component/kotoha.xml

# 3. ibus daemon 再起動
ibus restart

# 4. (任意) test 中に作成した learning_cache / user_vocab を初期化
#     production の DB と分けたい場合は §3.4 の path を別環境変数に切り出す
sqlite3 ~/.config/kotoha/kotoha.db \
  "DELETE FROM learning_cache WHERE last_used_at > strftime('%s','now','-1 day');"
```

---

## §11 完了判定とフォローアップ

### 完了判定

§8 の result doc が書き上がり、§1 の完了条件を全て満たした時点で **本手順 complete**。

### post-merge follow-up(global rule §post-merge follow-up checklist)

result doc を別 PR で merge 後:

- [ ] `docs/specs/_uncategorized/p3-a-ibus-engine.md` §13 Open Q 2(coalescing-window 値 empirical)に observation 反映
- [ ] `docs/ROADMAP.md` の Active milestone v0.3.0 table で #136 を `[x]`、Phase 3-B sub-milestone B6-c を **完了** マーク
- [ ] Phase 3 milestone v0.3.0 を「Active」→「完了済」へ移動
- [ ] annotated git tag `v0.3.0` を作成、リリース ADR `docs/adr/NNNN-release-v0.3.0.md` を起票
- [ ] (任意)glossary 同期 / vault sync は本手順で新規概念追加なしのため N/A 想定

### packaging task(別 ISSUE 化候補)

本 smoke procedure §4 では `~/.config/ibus/component/kotoha.xml` を手動配置している。production リリースでは:

- `kotoha.xml` を repo `assets/` に commit
- `cargo install --path crates/kotoha-bin` 等の install path に合わせた `<exec>` 値を template 化
- distribution package(deb / rpm / flatpak)で `/usr/share/ibus/component/kotoha.xml` に配置するスクリプト

これらは v0.3.0 milestone close 後の **packaging milestone**(v0.4.0 候補)で扱う。
