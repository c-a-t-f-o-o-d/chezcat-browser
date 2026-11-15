"""Original bounded writer for ChezCat's numeric icon/VERSIONINFO resources.

Development only; this is not a general RC compiler. Unsupported syntax fails
instead of being ignored. Format references are Microsoft's RESOURCEHEADER,
VERSIONINFO and PE/COFF specifications. GNU windres remains the normal alternate
path in build_icon_resources.py.
"""

import json
import re
import struct


def require(condition, message):
    if not condition:
        raise ValueError(message)


def aligned(data):
    return data + bytes((-len(data)) % 4)


def version_node(key, value=b"", children=(), text=False):
    body = aligned(bytes(6) + (key + "\0").encode("utf-16le")) + value
    for child in children:
        body = aligned(body) + child
    require(len(body) <= 65535, "Version block exceeds WORD length")
    size = len(value) // 2 if text else len(value)
    return struct.pack("<HHH", len(body), size, int(text)) + body[6:]


def rc_metadata(source):
    # Comments and this fixed grammar are intentionally the only RC subset.
    source = re.sub(r"//[^\n]*", "", source)
    pattern = re.compile(r'"(?:[^"\\]|\\.)*"|0[xX][0-9a-fA-F]+|[0-9]+|[A-Za-z_][A-Za-z_0-9]*|,')
    tokens = []
    previous = 0
    for item in pattern.finditer(source):
        require(not source[previous:item.start()].strip(), "Unsupported RC syntax")
        tokens.append(item.group())
        previous = item.end()
    require(not source[previous:].strip(), "Unsupported RC suffix")
    position = 0

    def take(expected=None):
        nonlocal position
        require(position < len(tokens), "Truncated RC resource")
        value = tokens[position]
        position += 1
        require(expected is None or value == expected, "Expected RC token " + str(expected))
        return value

    def number():
        value = take()
        require(re.fullmatch(r"(?:0[xX][0-9a-fA-F]+|[0-9]+)", value), "Expected RC number")
        return int(value, 16 if value.lower().startswith("0x") else 10)

    def string():
        value = take()
        require(value.startswith('"'), "Expected RC string")
        return json.loads(value.replace("\\0", "\\u0000"))

    take("LANGUAGE")
    primary = number()
    take(",")
    sublanguage = number()
    require(primary < 1024 and sublanguage < 64, "Invalid RC language")
    language = primary | sublanguage << 10
    icon_id = number()
    take("ICON")
    icon_path = string()
    version_id = number()
    take("VERSIONINFO")
    versions = []
    for label in ["FILEVERSION", "PRODUCTVERSION"]:
        take(label)
        values = [number()]
        for _ in range(3):
            take(",")
            values.append(number())
        require(all(0 <= value <= 65535 for value in values), "Invalid fixed version")
        versions.extend([values[0] << 16 | values[1], values[2] << 16 | values[3]])
    fixed = [0xFEEF04BD, 0x00010000] + versions
    for label in ["FILEFLAGSMASK", "FILEFLAGS", "FILEOS", "FILETYPE", "FILESUBTYPE"]:
        take(label)
        fixed.append(number())
    fixed.extend([0, 0])
    take("BEGIN")
    take("BLOCK")
    require(string() == "StringFileInfo", "Expected StringFileInfo")
    take("BEGIN")
    take("BLOCK")
    table = string()
    require(re.fullmatch(r"[0-9a-fA-F]{8}", table), "Invalid string-table language/codepage")
    take("BEGIN")
    strings = []
    names = set()
    while position < len(tokens) and tokens[position] == "VALUE":
        take("VALUE")
        name = string()
        take(",")
        value = string()
        require(name not in names and "\0" not in name, "Invalid/duplicate version key")
        require(value.endswith("\0") and "\0" not in value[:-1], "Expected one terminated version string")
        names.add(name)
        strings.append(version_node(name, value.encode("utf-16le"), text=True))
    require(1 <= len(strings) <= 32, "Version string count outside bound")
    take("END")
    take("END")
    take("BLOCK")
    require(string() == "VarFileInfo", "Expected VarFileInfo")
    take("BEGIN")
    take("VALUE")
    require(string() == "Translation", "Expected Translation")
    take(",")
    translation = number()
    take(",")
    codepage = number()
    take("END")
    take("END")
    require(position == len(tokens), "Unexpected additional RC resource")
    require(translation == language and int(table[:4], 16) == language
            and int(table[4:], 16) == codepage, "Language/codepage mismatch")
    require(1 <= icon_id <= 65535 and version_id == 1, "Invalid resource IDs")
    version = version_node("VS_VERSION_INFO", struct.pack("<13I", *fixed), [
        version_node("StringFileInfo", children=[version_node(table, children=strings, text=True)], text=True),
        version_node("VarFileInfo", children=[version_node("Translation", struct.pack("<HH", language, codepage))], text=True),
    ])
    return language, icon_id, icon_path, version


def resources(source, icon):
    require(len(source.encode("utf-8")) < 65536, "RC source exceeds bounded grammar size")
    language, group_id, path, version = rc_metadata(source)
    require(path == "assets/chezcat.ico", "Portable writer expects ChezCat icon path")
    require(6 <= len(icon) <= 16 * 1024 * 1024, "Icon input outside bound")
    reserved, kind, count = struct.unpack_from("<HHH", icon)
    require(reserved == 0 and kind == 1 and 1 <= count <= 256, "Invalid icon header")
    require(6 + count * 16 <= len(icon), "Truncated icon directory")
    result = {}
    group = struct.pack("<HHH", 0, 1, count)
    for index in range(count):
        at = 6 + index * 16
        size, start = struct.unpack_from("<II", icon, at + 8)
        require(size > 0 and 6 + count * 16 <= start <= len(icon) - size, "Invalid icon extent")
        result[(3, index + 1, language)] = icon[start:start + size]
        group += icon[at:at + 12] + struct.pack("<H", index + 1)
    result[(14, group_id, language)] = group
    result[(16, 1, language)] = version
    return result


def res_file(entries):
    def record(kind, name, language, value):
        # Preserve the original windres-generated .res flags for this RC input.
        flags = 0 if kind in (0, 16) else 0x1010
        header = struct.pack("<IIHHHHIHHII", len(value), 32, 0xFFFF, kind,
                             0xFFFF, name, 0, flags, language, 0, 0)
        return header + aligned(value)
    return record(0, 0, 0, b"") + b"".join(
        record(*key, value) for key, value in sorted(entries.items()))


def coff_file(entries):
    tree = {}
    for (kind, name, language), value in sorted(entries.items()):
        tree.setdefault(kind, {}).setdefault(name, {})[language] = value
    data = bytearray()
    leaves = []

    def directory(children):
        offset = len(data)
        data.extend(struct.pack("<IIHHHH", 0, 0, 0, 0, 0, len(children)))
        at = len(data)
        data.extend(bytes(8 * len(children)))
        for index, (key, child) in enumerate(sorted(children.items())):
            if isinstance(child, dict):
                target = directory(child) | 0x80000000
            else:
                target = 0
                leaves.append((at + index * 8 + 4, child))
            struct.pack_into("<II", data, at + index * 8, key, target)
        return offset

    directory(tree)
    payloads = []
    for target, value in leaves:
        entry = len(data)
        struct.pack_into("<I", data, target, entry)
        data.extend(struct.pack("<IIII", 0, len(value), 0, 0))
        payloads.append((entry, value))
    relocations = []
    for entry, value in payloads:
        data = bytearray(aligned(data))
        struct.pack_into("<I", data, entry, len(data))
        data.extend(value)
        relocations.append(struct.pack("<IIH", entry, 0, 3))  # AMD64 ADDR32NB
    data = aligned(data)
    relocation = 60 + len(data)
    symbols = relocation + 10 * len(relocations)
    header = struct.pack("<HHIIIHH", 0x8664, 1, 0, symbols, 1, 0, 4)
    section = struct.pack("<8sIIIIIIHHI", b".rsrc\0\0\0", 0, 0, len(data), 60,
                          relocation, 0, len(relocations), 0, 0x40300040)
    symbol = struct.pack("<8sIhHBB", b".rsrc\0\0\0", 0, 1, 0, 3, 0)
    return header + section + data + b"".join(relocations) + symbol + struct.pack("<I", 4)


def generate(source, icon):
    entries = resources(source, icon)
    return res_file(entries), coff_file(entries)
