#!/usr/bin/env python3
"""The field report: for each kind of the game's records, every offset the
decompiled code touches — with its width and type, how often it is read and
written, the routines that use it and the strings they show — the clues for
naming the fields.

Records are found two ways. Arrays in the game's globals are indexed with
their record size: `(&DAT_x)[i * size]`, `*(short *)(&DAT_x + i * size)`, or
through a variable that holds `i * size`. Records handed out by getters are
pointers: `*(short *)(p + 0x10)`, `p[8]`, `*p`, where `p` holds what a
getter returned, or the getter's call itself. Such pointers are followed
into the routines they are passed to and into the members of the objects
that keep them, until no more turn up.

    python3 tools/scripts/fields.py [--re ~/Programming/anstoss3-re]

reads `export/anstoss3.exe/` and `fields/names.toml` there, and writes
`fields/<kind>.md` and `fields/records.tsv`, which `ApplyRecords.java` turns
into structures in Ghidra.
"""

import argparse
import csv
import os
import re
import sys
import tomllib
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_RE = Path(os.environ.get("ANSTOSS3_RE") or Path.home() / "Programming/anstoss3-re")


@dataclass
class Kind:
    name: str
    title: str
    #: The structure's name in Ghidra.
    struct: str
    size: int | None
    #: Arrays of the records: base address and how many.
    arrays: list[tuple[int, int]] = field(default_factory=list)
    #: Functions that return a pointer to a record.
    getters: list[str] = field(default_factory=list)
    #: Functions that fill a record through `this` or ECX.
    readers: list[str] = field(default_factory=list)
    note: str = ""


KINDS = [
    Kind("players", "Players", "Player", 0x354, arrays=[(0x00C02550, 17000)], getters=["FUN_006968c4"],
         readers=["FUN_0068c257"],
         note="All players of the game; a club's own list holds pointers to them."),
    Kind("teams", "Teams", "Team", 0x990, arrays=[(0x01A72FF8, 0x10C)],
         note="The league teams' records, from which the club objects are filled."),
    Kind("managers", "Managers", "Manager", 0x2158, arrays=[(0x00A62598, 0)],
         note="The managers' records, numbered from 1."),
    Kind("clubs", "Club objects", "Club", None,
         getters=["FUN_0063dd1c", "FUN_0063b75a", "FUN_0063d5fa", "FUN_0063d949", "FUN_0063a88f"],
         note="What the club getter hands out: a cached object filled from the team records "
              "(and from those of other countries and national teams)."),
    Kind("manager-objects", "Manager objects", "ManagerObject", None, getters=["FUN_006a426b"],
         note="What the manager getter hands out: an object filled from the manager record."),
]

SIZES = {
    "char": 1, "uchar": 1, "byte": 1, "undefined": 1, "undefined1": 1, "bool": 1, "sbyte": 1,
    "short": 2, "ushort": 2, "undefined2": 2, "wchar_t": 2, "word": 2,
    "int": 4, "uint": 4, "undefined4": 4, "long": 4, "ulong": 4, "float": 4, "dword": 4,
    "double": 8, "undefined8": 8, "longlong": 8, "ulonglong": 8, "qword": 8,
}
SIGNED = {"char", "sbyte", "short", "int", "long", "longlong"}
UNSIGNED = {"uchar", "byte", "ushort", "uint", "ulong", "ulonglong", "word", "dword", "qword"}

HEADER = re.compile(r"^// ([0-9a-f]{8}) (\S+) \((\d+) bytes\)$", re.M)
CONST = r"(?:0x[0-9a-f]+|\d+)"
DECL = re.compile(r"^\s+([A-Za-z_][\w ]*?)\s*(\**)\s*(\w+)(?:\s*\[\w*\])?;$", re.M)
DEREF_OPEN = re.compile(r"\*\(([A-Za-z_][\w ]*?)\s*(\*+)\)\(\s*(\(int\)\s*)?$")
DEREF_BARE = re.compile(r"\*\(([A-Za-z_][\w ]*?)\s*(\*+)\)\s*$")
STAR = re.compile(r"(?:^|[=(,;{!&|?:]|return)\s*\*\s*$")


@dataclass
class Access:
    kind: str
    offset: int
    width: int | None
    type: str
    how: str  # read, write, update, address
    routine: str
    table: str = ""


@dataclass
class Facts:
    """What holds a record beyond the getters' results: parameters of
    routines and members of classes, by kind. A parameter holds one when
    every call passes a record of that kind (or nothing), a member when
    every store into it does."""
    params: dict = field(default_factory=dict)  # (routine, parameter) -> kind
    members: dict = field(default_factory=dict)  # (class, offset) -> kind


@dataclass
class Seen:
    """What one round saw passed and stored: the kinds, or None for values
    that are not known records."""
    params: dict = field(default_factory=lambda: defaultdict(set))
    members: dict = field(default_factory=lambda: defaultdict(set))

    def facts(self) -> Facts:
        only = lambda seen: {k: next(iter(v)) for k, v in seen.items() if len(v) == 1 and None not in v}
        return Facts(only(self.params), only(self.members))


def main() -> int:
    args = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    args.add_argument("--re", type=Path, default=DEFAULT_RE)
    re_dir = args.parse_args().re
    export = re_dir / "export/anstoss3.exe"
    functions = load_functions(export / "decompiled")
    strings = {(r["name"] if r["namespace"] == "_global" else f"{r['namespace']}::{r['name']}"):
               r["strings"] for r in csv.DictReader(
                   open(export / "functions.tsv", encoding="utf-8"), delimiter="\t")}
    names_file = re_dir / "fields/names.toml"
    names = tomllib.loads(names_file.read_text(encoding="utf-8")) if names_file.exists() else {}

    facts = Facts()
    routines = list({id(f): f for f in functions.values()}.values())
    for round_ in range(1, 11):
        accesses, dynamic, seen = [], Counter(), Seen()
        for f in routines:
            found, unknown = scan(f, functions, facts, seen)
            accesses += found
            dynamic.update(unknown)
        learned = seen.facts()
        print(f"round {round_}: {len(accesses)} accesses; {len(learned.params)} parameters and "
              f"{len(learned.members)} members hold records")
        if learned == facts:
            break
        facts = learned

    (re_dir / "fields").mkdir(exist_ok=True)
    rows, ok = [], True
    for kind in KINDS:
        mine = [a for a in accesses if a.kind == kind.name]
        known = {int(k, 16): v for k, v in names.get(kind.name, {}).items()}
        report, fields_ = write_report(kind, mine, dynamic[kind.name], known, strings, functions)
        (re_dir / f"fields/{kind.name}.md").write_text(report, encoding="utf-8")
        rows += [(kind, *f) for f in fields_]
        missing = [k for k in known if not any(a.offset == k for a in mine)]
        offsets = len({a.offset for a in mine})
        print(f"{kind.name:16} {len(mine):6} accesses at {offsets:4} offsets in "
              f"{len({a.routine for a in mine}):4} routines; {len(known)} named"
              + (f", not found: {', '.join(hex(m) for m in missing)}" if missing else ""))
        ok &= not missing
    write_records(re_dir / "fields/records.tsv", rows)
    print(f"wrote {re_dir / 'fields'}")
    return 0 if ok else 1


@dataclass
class Function:
    name: str
    address: str
    size: int
    signature: str
    body: str
    #: Declared type of each local and parameter: base type and pointer depth.
    types: dict[str, tuple[str, int]]
    returns: tuple[str, int]
    params: list[str]

    @property
    def cls(self) -> str:
        return self.name.rsplit("::", 1)[0] if "::" in self.name else ""


def load_functions(decompiled: Path) -> dict[str, Function]:
    functions = {}
    for path in sorted(decompiled.glob("*.c")):
        text = path.read_text(encoding="utf-8", errors="replace")
        heads = list(HEADER.finditer(text))
        for i, head in enumerate(heads):
            end = heads[i + 1].start() if i + 1 < len(heads) else len(text)
            chunk = text[head.end():end]
            open_brace = chunk.find("{")
            if open_brace < 0:
                continue
            signature = " ".join(chunk[:open_brace].split())
            body = chunk[open_brace:]
            types, names = {}, []
            params = re.search(r"\((.*)\)\s*$", signature)
            for param in (params.group(1).split(",") if params else []):
                m = re.match(r"\s*([A-Za-z_][\w ]*?)\s*(\**)\s*(\w+)\s*$", param)
                names.append(m.group(3) if m else "")
                if m:
                    types[m.group(3)] = (m.group(1).strip(), len(m.group(2)))
            for m in DECL.finditer(body[: body.find("\n\n", 2) + 1 or None]):
                types[m.group(3)] = (m.group(1).strip(), len(m.group(2)))
            ret = re.match(r"\s*([A-Za-z_][\w ]*?)\s*(\**)\s*(?:__\w+\s+)?[\w:~]+\(", signature)
            returns = (ret.group(1).strip(), len(ret.group(2))) if ret else ("undefined4", 0)
            functions[head.group(2)] = Function(head.group(2), head.group(1), int(head.group(3)),
                                                signature, body, types, returns, names)
    # Calls name a routine without its class when it is global.
    short = Counter(name.split("::")[-1] for name in functions)
    for name, f in list(functions.items()):
        if "::" in name and short[name.split("::")[-1]] == 1:
            functions.setdefault(name.split("::")[-1], f)
    return functions


def size_of(type_: str) -> int | None:
    return SIZES.get(type_.split()[-1] if type_ else "")


def signedness(type_: str) -> str:
    base = type_.split()[-1] if type_ else ""
    if base in SIGNED:
        return "signed"
    if base in UNSIGNED:
        return "unsigned"
    if base in ("float", "double"):
        return "float"
    return ""


def group_end(text: str, start: int, open_: str, close: str) -> int:
    """Index just past the bracket that closes the one at `start`."""
    depth = 0
    for i in range(start, len(text)):
        if text[i] == open_:
            depth += 1
        elif text[i] == close:
            depth -= 1
            if depth == 0:
                return i + 1
    return len(text)


def terms(expr: str) -> list[str]:
    """The top-level terms of a sum, with their signs."""
    out, depth, current = [], 0, ""
    for c in expr:
        if c in "([":
            depth += 1
        elif c in ")]":
            depth -= 1
        if depth == 0 and c in "+-" and current.strip() and not current.rstrip().endswith(("*", "(")):
            out.append(current)
            current = c
        else:
            current += c
    if current.strip():
        out.append(current)
    return [t.strip() for t in out]


def constant(term: str) -> int | None:
    t = term.replace(" ", "")
    sign = -1 if t.startswith("-") else 1
    t = t.lstrip("+-")
    t = re.sub(r"^\(\w+\)", "", t)  # a cast
    return sign * int(t, 0) if re.fullmatch(CONST, t) else None


def how_used(text: str, end: int) -> str:
    after = text[end:end + 4]
    if re.match(r"\s*=(?!=)", after):
        return "write"
    if re.match(r"\s*([-+*/|&^]|<<|>>)=", after) or re.match(r"\s*(\+\+|--)", after):
        return "update"
    return "read"


THIS = r"(?:this|extraout_ECX|in_ECX)"
MEMBER = re.compile(r"\*\((?:int|uint|undefined4|void \*|\w+ \*)\s*\*\)\(%s \+ (%s)\)" % (THIS, CONST))


def token(kind: str, what: str) -> str:
    return f"RECORD_{kind.replace('-', '_')}__{what}"


def scan(f: Function, functions, facts: Facts, seen: Seen) -> tuple[list[Access], Counter]:
    body = f.body
    accesses, dynamic = [], Counter()
    getters = {g: k for k in KINDS for g in k.getters}
    types = dict(f.types)
    # Inline getter calls become tokens, so they read like variables, and so
    # do the members of this object that hold records.
    for getter, kind in getters.items():
        while (m := re.search(r"\b(?:\w+::)?%s\(" % getter, body)):
            end = group_end(body, m.end() - 1, "(", ")")
            body = body[:m.start()] + token(kind.name, getter) + body[end:]
            types[token(kind.name, getter)] = functions[getter].returns if getter in functions else ("undefined4", 0)
    if f.cls:
        def member(m):
            kind = facts.members.get((f.cls, int(m.group(1), 0)))
            # Stores into the member stay as they are: they are what shows
            # it holds a record.
            if not kind or re.match(r"\s*=(?!=)", m.string[m.end():m.end() + 3]):
                return m.group(0)
            name = token(kind, f"member_{m.group(1)}")
            types[name] = ("int", 0)
            return name
        body = MEMBER.sub(member, body)

    # Locals that hold a record, from a getter or an array's address, and
    # locals that hold an index scaled by a record size.
    holders: dict[str, tuple[str, int]] = {}
    scaled: dict[str, tuple[Kind, int]] = {}
    for m in re.finditer(r"(?m)^\s*(\w+) = (.+);$", body):
        var, value = m.group(1), m.group(2)
        bare = re.sub(r"^\([\w ]+\*+\)\s*", "", value.strip())
        got = re.fullmatch(r"RECORD_(\w+?)__\w+", bare)
        if got:
            holders[var] = (got.group(1).replace("_", "-"), 0)
            continue
        for kind in KINDS:
            if kind.size is None:
                continue
            mult = re.search(r"\*\s*(%s)\s*$" % CONST, value)
            if mult and int(mult.group(1), 0) in stride_units(kind.size):
                scaled[var] = (kind, int(mult.group(1), 0))
            pointed = re.match(r"&DAT_([0-9a-f]{8})\s*\+\s*(.+)$", bare)
            if pointed and (field_ := in_array(kind, int(pointed.group(1), 16))) is not None:
                if stride_of(pointed.group(2), kind, scaled):
                    holders[var] = (kind.name, field_)
    holders = {v: h for v, h in holders.items() if v not in scaled}
    for param in f.params:
        if (kind := facts.params.get((f.name, param))) and param not in holders:
            holders[param] = (kind, 0)
    for kind in KINDS:
        if f.name.split("::")[-1] in kind.readers:
            for var in ("this", "in_ECX", "extraout_ECX"):
                holders.setdefault(var, (kind.name, 0))

    # What this routine tells about others: the records it passes on, and
    # those it keeps in its object's members.
    def record_in(expr: str) -> str | None | bool:
        """The kind of record an expression is, None if it is something
        else, False for nothing at all."""
        bare = re.sub(r"^(\((?:[\w ]+\*+|int|uint)\)\s*)+", "", expr.strip())
        if bare in ("0", "(void *)0", "NULL"):
            return False
        if (t := re.fullmatch(r"RECORD_(\w+?)__\w+", bare)):
            return t.group(1).replace("_", "-")
        if bare in holders and holders[bare][1] == 0:
            return holders[bare][0]
        return None

    if f.cls:
        for m in re.finditer(r"\*\([\w ]+\*\)\(%s \+ (%s)\) = ([^;]+);" % (THIS, CONST), body):
            if (kind := record_in(m.group(2))) is not False:
                seen.members[(f.cls, int(m.group(1), 0))].add(kind)
    for m in re.finditer(r"\b((?:\w+::)?FUN_[0-9a-f]{8})\(", body):
        callee = functions.get(m.group(1)) or functions.get(m.group(1).split("::")[-1])
        if not callee:
            continue
        end = group_end(body, m.end() - 1, "(", ")")
        for param, arg in zip(callee.params, split_args(body[m.end():end - 1])):
            if param and (kind := record_in(arg)) is not False:
                seen.params[(callee.name, param)].add(kind)

    # Pointer records.
    for var, (kind, adjust) in holders.items():
        pointee = types.get(var, ("undefined4", 0))
        element = size_of(pointee[0]) or 1 if pointee[1] > 0 else 1
        if pointee[1] > 1:
            element = 4
        for m in re.finditer(r"\b%s\b" % re.escape(var), body):
            access = pointer_access(body, m.start(), m.end(), element, kind, adjust, f.name)
            if access:
                accesses.append(access)
    for m in re.finditer(r"RECORD_(\w+?)__(\w+)", body):
        kind = m.group(1).replace("_", "-")
        pointee = types.get(m.group(0), ("undefined4", 0))
        element = (size_of(pointee[0]) or 1) if pointee[1] > 0 else 1
        access = pointer_access(body, m.start(), m.end(), element, kind, 0, f.name)
        if access:
            accesses.append(access)

    # Array records.
    for m in re.finditer(r"&DAT_([0-9a-f]{8})", body):
        address = int(m.group(1), 16)
        for kind in KINDS:
            if kind.size is None or (field_ := in_array(kind, address)) is None:
                continue
            access = array_access(body, m.start(), m.end(), kind, field_, scaled, f.name)
            if access == "dynamic":
                dynamic[kind.name] += 1
            elif access:
                accesses.append(access)
    return accesses, dynamic


def split_args(args: str) -> list[str]:
    out, depth, current = [], 0, ""
    for c in args:
        if c in "([":
            depth += 1
        elif c in ")]":
            depth -= 1
        if c == "," and depth == 0:
            out.append(current)
            current = ""
        else:
            current += c
    return out + [current] if current.strip() else out


def stride_units(size: int) -> set[int]:
    return {size // w for w in (1, 2, 4, 8) if size % w == 0}


def in_array(kind: Kind, address: int) -> int | None:
    """The field of an array of `kind` an address lies in, if it does."""
    for base, count in kind.arrays:
        top = base + kind.size * (count or 1)
        if base - kind.size <= address < top:
            return (address - base) % kind.size
    return None


def stride_of(expr: str, kind: Kind, scaled) -> int | None:
    """The width an index expression steps a record array by: 1 for byte
    offsets, 2 for an index scaled to shorts, …; None if it has no index."""
    for m in re.finditer(r"\*\s*(%s)\b" % CONST, expr):
        unit = int(m.group(1), 0)
        if unit in stride_units(kind.size):
            return kind.size // unit
    for var, (k, unit) in scaled.items():
        if k is kind and re.search(r"\b%s\b" % re.escape(var), expr):
            return kind.size // unit
    return None


def other_terms(expr: str, kind: Kind, scaled) -> tuple[int, bool]:
    """The constant the index expression adds, and whether it adds more."""
    extra, rest = 0, False
    for t in terms(expr):
        if stride_of(t, kind, scaled):
            continue
        value = constant(t)
        if value is None:
            rest = True
        else:
            extra += value
    return extra, rest


def array_access(body, start, end, kind, field_, scaled, routine):
    before = body[max(0, start - 60):start]
    deref = DEREF_OPEN.search(before)
    if before.endswith("(") and body.startswith(")[", end):
        # (&DAT_x)[index]: the element is as wide as the index scales.
        close = group_end(body, end + 1, "[", "]")
        index = body[end + 2:close - 1]
        width = stride_of(index, kind, scaled)
        if not width:
            return None
        extra, rest = other_terms(index, kind, scaled)
        if rest:
            return "dynamic"
        how = "address" if body[start - 2:start - 1] == "&" else how_used(body, close)
        return Access(kind.name, (field_ + extra * width) % kind.size, width,
                      {1: "undefined1", 2: "undefined2", 4: "undefined4"}.get(width, ""), how, routine)
    if deref and not deref.group(3):
        # *(T *)(&DAT_x + index)
        open_ = start - (len(before) - before.rfind("("))
        close = group_end(body, open_, "(", ")")
        expr = body[end:close - 1].lstrip(" +")
        if not stride_of(expr, kind, scaled):
            return None
        extra, rest = other_terms(expr, kind, scaled)
        if rest:
            return "dynamic"
        type_ = deref.group(1).strip()
        width = size_of(type_) if len(deref.group(2)) == 1 else 4
        return Access(kind.name, (field_ + extra) % kind.size, width, type_,
                      how_used(body, close), routine)
    # &DAT_x + index, not looked into here: the address of a field.
    after = re.match(r"\s*\+\s*([^,;)]+(?:\([^)]*\)[^,;)]*)*)", body[end:end + 120])
    if after and stride_of(after.group(1), kind, scaled):
        extra, rest = other_terms(after.group(1), kind, scaled)
        if not rest:
            return Access(kind.name, (field_ + extra) % kind.size, None, "", "address", routine)
    return None


def pointer_access(body, start, end, element, kind, adjust, routine):
    before = body[max(0, start - 60):start]
    after = body[end:end + 160]
    deref = DEREF_OPEN.search(before)
    if deref:
        byte_wise = bool(deref.group(3))
        step = 1 if byte_wise else element
        open_ = start - len(before) + before.rfind("(", 0, len(before) - (len(deref.group(3) or "")))
        close = group_end(body, open_, "(", ")")
        expr = body[end:close - 1]
        offset, table = 0, ""
        for t in terms(expr):
            value = constant(t)
            if value is not None:
                offset += value * step
            elif t.strip():
                table = t.strip(" +")
        type_ = deref.group(1).strip()
        width = size_of(type_) if len(deref.group(2)) == 1 else 4
        return Access(kind, adjust + offset, width, type_, how_used(body, close), routine, table)
    bare = DEREF_BARE.search(before)
    if bare and not re.match(r"\s*(\+|\[)", after):
        type_ = bare.group(1).strip()
        width = size_of(type_) if len(bare.group(2)) == 1 else 4
        return Access(kind, adjust, width, type_, how_used(body, end), routine)
    index = re.match(r"\[(%s)\]" % CONST, after)
    if index:
        return Access(kind, adjust + int(index.group(1), 0) * element, element, "",
                      how_used(body, end + index.end()), routine)
    if STAR.search(before) and not re.match(r"\s*(\+|\[)", after):
        return Access(kind, adjust, element, "", how_used(body, end), routine)
    plus = re.match(r"\s*\+\s*(%s)\b" % CONST, after)
    if plus and re.search(r"\(\s*(\(int\)\s*)?$", before):
        return Access(kind, adjust + int(plus.group(1), 0) * element, None, "", "address", routine)
    return None


def write_report(kind, accesses, dynamic, known, strings, functions):
    # Offsets past a record of known size reach other data through it.
    outside = [a for a in accesses if kind.size and not 0 <= a.offset < kind.size]
    accesses = [a for a in accesses if a not in outside]
    by_offset = defaultdict(list)
    for a in accesses:
        by_offset[a.offset].append(a)
    size = f"{kind.size:#x} bytes" if kind.size else "size unknown"
    where = [f"{count or 'some'} at `DAT_{base:08x}`" for base, count in kind.arrays]
    where += [f"from `{g}`" for g in kind.getters]
    lines = [
        f"# {kind.title}",
        "",
        "Generated by `tools/scripts/fields.py` of the replica from `export/anstoss3.exe/`;",
        "names come from `names.toml`. Ascaron's code, decompiled: for study only.",
        "",
        f"{kind.note} {size}; " + "; ".join(where) + ".",
        "",
        f"{len(accesses)} accesses at {len(by_offset)} offsets in "
        f"{len({a.routine for a in accesses})} routines"
        + (f"; {dynamic} more at offsets computed at run time" if dynamic else "")
        + (f"; {len(outside)} past the record's end, left out" if outside else "") + ".",
        "",
        "| offset | name | width | type | reads | writes | routines | strings |",
        "|---:|---|---|---|---:|---:|---|---|",
    ]
    fields_ = []
    for offset in sorted(set(by_offset) | set(known)):
        found = by_offset.get(offset, [])
        widths = Counter(a.width for a in found if a.width)
        types = Counter(a.type for a in found if a.type and not a.type.startswith("undefined"))
        reads = sum(a.how in ("read", "update") for a in found)
        writes = sum(a.how in ("write", "update") for a in found)
        taken = sum(a.how == "address" for a in found)
        tables = sorted({a.table for a in found if a.table})
        routines = Counter(a.routine for a in found)
        # Small routines say most about what they do with the field.
        ordered = sorted(routines, key=lambda r: (functions[r].size, r))
        shown = ", ".join(f"`{r}`" for r in ordered[:4])
        if len(ordered) > 4:
            shown += f" +{len(ordered) - 4}"
        clues = []
        for r in ordered[:3]:
            for s in (strings.get(r) or "").split(" | "):
                if s and s not in clues:
                    clues.append(s if len(s) <= 40 else s[:37] + "…\"")
        name = known.get(offset, {}).get("name", "")
        width = widths.most_common(1)[0][0] if widths else None
        type_ = types.most_common(1)[0][0] if types else ""
        notes = []
        if taken:
            notes.append(f"{taken}× address")
        if tables:
            notes.append("indexed by " + ", ".join(f"`{t}`" for t in tables[:2]))
        lines.append(
            f"| {offset:#05x} | {name} | {'/'.join(str(w) for w in sorted(widths)) or ''} "
            f"| {type_ or signedness(type_)} | {reads} | {writes} | {shown}"
            + (" (" + "; ".join(notes) + ")" if notes else "")
            + f" | {' '.join(clues[:3]).replace('|', '/')} |")
        fields_.append((offset, width or known.get(offset, {}).get("width", 1), name, type_))
    return "\n".join(lines) + "\n", fields_


def write_records(path: Path, rows) -> None:
    """One line per field, for ApplyRecords.java: the structure, its size
    (0 when unknown), arrays and getters, and the field's offset, width,
    name and type."""
    with open(path, "w", encoding="utf-8", newline="") as out:
        w = csv.writer(out, delimiter="\t", lineterminator="\n")
        w.writerow(["struct", "size", "arrays", "getters", "offset", "width", "name", "type"])
        for kind, offset, width, name, type_ in rows:
            if kind.size and not 0 <= offset < kind.size:
                continue
            arrays = ",".join(f"{base:08x}:{count}" for base, count in kind.arrays)
            w.writerow([kind.struct, kind.size or 0, arrays, ",".join(kind.getters),
                        f"{offset:#x}", width, name, type_])


if __name__ == "__main__":
    sys.exit(main())
