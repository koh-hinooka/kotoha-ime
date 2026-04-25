"""Targeted 30 cases for Phase 2 golden fixture (spec §6.3.1)."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Case:
    input: str
    expected_top_surface: str
    category: str
    note: str


TARGETED_CASES: list[Case] = [
    # 敬称 10
    Case("やまださん", "山田さん", "honorific", "敬称+人名"),
    Case("たなかさん", "田中さん", "honorific", "敬称+人名"),
    Case("すずきせんせい", "鈴木先生", "honorific", "敬称+教職"),
    Case("さとうさま", "佐藤様", "honorific", "尊敬敬称"),
    Case("ほんださん", "本田さん", "honorific", "敬称+人名"),
    Case("なかむらくん", "中村くん", "honorific", "男性敬称"),
    Case("やまもとちゃん", "山本ちゃん", "honorific", "親愛敬称"),
    Case("こばやしせんぱい", "小林先輩", "honorific", "年長敬称"),
    Case("おがわかちょう", "小川課長", "honorific", "役職敬称"),
    Case("いしいしゃちょう", "石井社長", "honorific", "役職敬称"),
    # 人名 10
    Case("やまだたろう", "山田太郎", "personal_name", "姓+名"),
    Case("すずきはなこ", "鈴木花子", "personal_name", "姓+名"),
    Case("たなかいちろう", "田中一郎", "personal_name", "姓+名"),
    Case("さとうけんじ", "佐藤健二", "personal_name", "姓+名"),
    Case("わたなべゆき", "渡辺由紀", "personal_name", "姓+名"),
    Case("いとうまさし", "伊藤正", "personal_name", "姓+名"),
    Case("やまもとあきら", "山本明", "personal_name", "姓+名"),
    Case("なかむらみさき", "中村美咲", "personal_name", "姓+名"),
    Case("こばやしゆうじ", "小林雄二", "personal_name", "姓+名"),
    Case("まつもとなおこ", "松本直子", "personal_name", "姓+名"),
    # 地名・組織名 5
    Case("とうきょうと", "東京都", "place_org", "地名"),
    Case("おおさかふ", "大阪府", "place_org", "地名"),
    Case("ほっかいどう", "北海道", "place_org", "地名"),
    Case("きょうとだいがく", "京都大学", "place_org", "組織名"),
    Case("とよたじどうしゃ", "トヨタ自動車", "place_org", "組織名"),
    # 外来語 5
    Case("こんぴゅーたー", "コンピューター", "loanword", "長音付き"),
    Case("ぷろぐらむ", "プログラム", "loanword", "促音なし"),
    Case("さーばー", "サーバー", "loanword", "長音付き"),
    Case("かめら", "カメラ", "loanword", "短い外来語"),
    Case("すまーとふぉん", "スマートフォン", "loanword", "長音+合成語"),
]


def targeted_as_tsv_rows() -> list[str]:
    """Return targeted cases as TSV lines (input<TAB>expected<TAB>category<TAB>note)."""
    return [
        f"{c.input}\t{c.expected_top_surface}\t{c.category}\t{c.note}"
        for c in TARGETED_CASES
    ]
