# Kotoha Roadmap

Kotoha プロジェクトの開発フェーズと、各フェーズの到達目標を記録する。

## Phase 一覧

| Phase | 名称 | 内容 | 状態 |
|---|---|---|---|
| 0 | Foundation | Cargo workspace + ローマ字→かな変換 + 入力モード管理 + CLI | 実装中 |
| 1 | Kana→Kanji conversion | Zenz + llama.cpp によるかな→漢字変換 | 未着手 |
| 2 | Dictionary and learning | システム辞書 + ユーザ辞書 + 学習キャッシュ | 未着手 |
| 3 | IBus integration | IBus engine(GNOME Mutter 用) | 未着手 |
| 4 | fcitx5 integration | fcitx5 addon(KDE / wlroots 用) | 未着手 |
| 5 | Advanced features | タイポ訂正 + 文脈リランキング | 未着手 |
| 6 | UX polish | 設定 UI + 辞書自動更新 + 同期 | 未着手 |

## Phase 0 マイルストーン

Phase 0 の詳細マイルストーン分割は `docs/superpowers/plans/2026-04-22-kotoha-phase-0-implementation.md` を参照。

## Phase 1 への申し送り

Phase 0 完了時に Phase 1 へ引き継ぐ設計判断事項を記録する。

- **canonical romaji ADR** (`docs/adr/0008-canonical-romaji-and-partial-invertibility.md`): Phase 0 の spec §11.3 revision 2 で除外した「可逆性」プロパティについて、canonical romaji を定義して部分的可逆性を回復する拡張を Phase 1 で検討する。本 ADR はステータス「提案」の placeholder であり、Phase 1 計画開始時に 3 選択肢 (現状維持 / canonical 定義 / configuration で選択) から決定して正式 ADR に昇格する。

## 注記

- Phase 5 の「Shift 挙動設定」は、設計書 revision 2 の判断により Phase 0 に前倒しされた。Phase 5 は「タイポ訂正 + 文脈リランキング」のみ。
- 各 Phase の設計書は `docs/superpowers/specs/` に配置する。
