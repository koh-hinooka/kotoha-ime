# #36: Phase 0 spec narrative — ADR 総数 3→4 の更新

## 実施内容

- spec `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` の narrative 3 箇所を "ADR 3 件 (0001, 0002, 0003)" → "ADR 4 件 (0001, 0002, 0003, 0004)" に更新。canonical scheme は 0001 retraction / 0002 input-mode / 0003 shift / 0004 cli-line (ISSUE #34 / PR #35 で合意済み)。
- 編集箇所: §5 file-tree コメント (line 124)、§14 工数目安テーブル (line 672、見積もりも 0.3 日 → 0.4 日 に比例スケール)、§16 改版履歴 (line 748)。
- §14 合計行 "約 9〜10 日" は 0.1 日の増分が "約" の範囲内に収まるため据え置き。
- team-review (security + architecture + testing、inline fallback) は blocking finding ゼロ。Medium 1 件は §13.3 deliverable checklist が 0002/0003/0004 のみで 0001 を欠く件だが、narrative 総数ではなく specific-citation リストであり #36 scope 外。PR body で follow-up 候補として明示。

## つまずき

- 当初 ISSUE body は line 672 / 748 の 2 箇所のみを明示していたが、事前 grep で §5 file-tree コメント (line 124) の "0001, 0002, 0003 を Phase 0 で作成" も narrative reference と判定し、ISSUE 本文の "any other narrative references" 条項に従って同一 PR に含めた。3 箇所で完結。
- §13.3 (lines 645–647) の deliverable checklist が 0002/0003/0004 のみで 0001 を欠いていることを発見。これは specific-citation (各 ADR を filename で個別列挙) であって count narrative ではないため、ISSUE #36 scope の "narrative references" に該当しないと判断し、PR body に follow-up 候補として記載して本 PR では触れず。

## 成果物リンク

- ISSUE: #36
- PR: #42 (merge commit: `6393a0e60ca8738b9b2c809bd08d2d5d7dc7e1e4`)
- spec edit: `docs/superpowers/specs/2026-04-22-kotoha-phase-0-design.md` §5 / §14 / §16
- follow-up 候補: §13.3 deliverable checklist への ADR 0001 追加 (別 ISSUE で起票するかは未定)
