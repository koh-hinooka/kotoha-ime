"""QWERTY-adjacent typo injection.

This module injects reproducible typos into an ASCII romaji string using
one of four operations: substitute, transpose, delete, insert. The edit
distance equals the number of operations applied. All random choices go
through an injected :class:`random.Random` instance so the output is
deterministic under a given seed.

Design choices:
- The adjacency table covers US-QWERTY letter keys plus ``space``, ``,``,
  and ``.``. Digits and shift-modified symbols are intentionally omitted
  because the generated romaji in this PoC is lowercase ASCII only.
- ``substitute`` picks a replacement uniformly from the target key's
  adjacency list.
- ``transpose`` swaps two adjacent characters; it is only attempted when
  the string has length >= 2.
- ``delete`` removes a single character; it is only attempted when the
  string has length >= 1.
- ``insert`` picks a neighbour of the character that currently sits at
  the insertion index (or uses ``a`` if the string is empty) and inserts
  it before that position.
- If an operation becomes impossible (for example ``transpose`` on a
  single character), the injector falls back to ``substitute`` so the
  requested edit distance is still reached.
"""

import random
from collections.abc import Callable
from typing import Final, Literal

TypoOp = Literal["substitute", "transpose", "delete", "insert"]

_OPS: Final[tuple[TypoOp, ...]] = ("substitute", "transpose", "delete", "insert")


# US-QWERTY physical adjacency (lowercase letters + space / , / .).
# Each entry lists the keys that are directly reachable by a single
# mistyped keystroke.
QWERTY_ADJACENCY: Final[dict[str, tuple[str, ...]]] = {
    "q": ("w", "a"),
    "w": ("q", "e", "a", "s"),
    "e": ("w", "r", "s", "d"),
    "r": ("e", "t", "d", "f"),
    "t": ("r", "y", "f", "g"),
    "y": ("t", "u", "g", "h"),
    "u": ("y", "i", "h", "j"),
    "i": ("u", "o", "j", "k"),
    "o": ("i", "p", "k", "l"),
    "p": ("o", "l"),
    "a": ("q", "w", "s", "z"),
    "s": ("w", "e", "a", "d", "z", "x"),
    "d": ("e", "r", "s", "f", "x", "c"),
    "f": ("r", "t", "d", "g", "c", "v"),
    "g": ("t", "y", "f", "h", "v", "b"),
    "h": ("y", "u", "g", "j", "b", "n"),
    "j": ("u", "i", "h", "k", "n", "m"),
    "k": ("i", "o", "j", "l", "m"),
    "l": ("o", "p", "k"),
    "z": ("a", "s", "x"),
    "x": ("z", "s", "d", "c"),
    "c": ("x", "d", "f", "v"),
    "v": ("c", "f", "g", "b"),
    "b": ("v", "g", "h", "n"),
    "n": ("b", "h", "j", "m"),
    "m": ("n", "j", "k"),
    " ": ("c", "v", "b", "n", "m"),
    ",": ("m", "k", "l", "."),
    ".": (",", "l"),
}


def _neighbor(ch: str, rng: random.Random) -> str:
    neighbors = QWERTY_ADJACENCY.get(ch)
    if not neighbors:
        # Unknown character: fall back to a random lowercase letter so the
        # typo operation still produces a change.
        return rng.choice("abcdefghijklmnopqrstuvwxyz")
    return rng.choice(neighbors)


def _substitute(text: str, rng: random.Random) -> str:
    if not text:
        return _neighbor("a", rng)
    idx = rng.randrange(len(text))
    original = text[idx]
    replacement = _neighbor(original, rng)
    return text[:idx] + replacement + text[idx + 1 :]


def _transpose(text: str, rng: random.Random) -> str:
    if len(text) < 2:
        return _substitute(text, rng)
    idx = rng.randrange(len(text) - 1)
    chars = list(text)
    chars[idx], chars[idx + 1] = chars[idx + 1], chars[idx]
    return "".join(chars)


def _delete(text: str, rng: random.Random) -> str:
    """Delete one random character from ``text``.

    Falls back to :func:`_substitute` when ``text`` is empty, because edit
    distance semantics require an actual mutation and deletion on an empty
    string is a no-op.

    Note:
        In multi-step operation chains (``distance >= 2``), if sequential
        ``_delete`` calls empty the string, subsequent operations invoke
        the ``_substitute`` fallback. The nominal ``distance`` parameter
        therefore counts the number of operations applied, not the exact
        Levenshtein edit distance between input and final output. The
        fallback is acceptable for the PoC because it preserves
        determinism and never raises; a strict distance-preserving
        operation graph is deferred to Phase 5 kick-off.

    Args:
        text: Source string to delete a character from.
        rng: Seeded random number generator.

    Returns:
        A new string with exactly one character removed when
        ``len(text) >= 1``; otherwise the result of the substitute
        fallback.
    """
    if not text:
        return _substitute(text, rng)
    idx = rng.randrange(len(text))
    return text[:idx] + text[idx + 1 :]


def _insert(text: str, rng: random.Random) -> str:
    if not text:
        return _neighbor("a", rng)
    idx = rng.randrange(len(text) + 1)
    anchor = text[idx] if idx < len(text) else text[-1]
    ch = _neighbor(anchor, rng)
    return text[:idx] + ch + text[idx:]


_TypoFn = Callable[[str, random.Random], str]

_DISPATCH: Final[dict[TypoOp, _TypoFn]] = {
    "substitute": _substitute,
    "transpose": _transpose,
    "delete": _delete,
    "insert": _insert,
}


def inject_typos(text: str, distance: int, seed: int) -> str:
    """Inject ``distance`` QWERTY-adjacent typos into ``text``.

    Args:
        text: Input ASCII string to corrupt.
        distance: Number of edit operations to apply (0-3 in practice).
            ``0`` returns ``text`` unchanged. This is the nominal
            operation count, not the strict Levenshtein edit distance
            between input and output: when an operation cannot proceed
            (e.g. ``_transpose`` on a single character, ``_delete`` on
            an already-empty string), the injector falls back to
            ``_substitute``, which can make the resulting Levenshtein
            distance smaller than ``distance``. The fallback preserves
            determinism; exact edit-distance guarantees are deferred to
            Phase 5 kick-off.
        seed: Deterministic seed; the same ``(text, distance, seed)``
            triple always produces the same output.

    Returns:
        The corrupted string. Each operation is chosen uniformly from
        ``substitute`` / ``transpose`` / ``delete`` / ``insert`` using the
        supplied ``seed`` via :class:`random.Random`.

    Raises:
        ValueError: If ``distance`` is negative.
    """
    if distance < 0:
        raise ValueError(f"distance must be non-negative, got {distance}")
    if distance == 0:
        return text
    rng = random.Random(seed)
    current = text
    for _ in range(distance):
        op = rng.choice(_OPS)
        current = _DISPATCH[op](current, rng)
    return current
