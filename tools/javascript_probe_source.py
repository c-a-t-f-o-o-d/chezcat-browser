"""Render a standalone Rust probe against the selected ChezCat source revision."""
import json


def write_probe(template, source_root, destination):
    src = source_root.resolve() / "src"
    javascript = src / "javascript.rs"
    engine = javascript.read_text(encoding="utf-8")
    names = []
    if "crate::canvas_2d_bitmap::" in engine:
        names.extend(("canvas_2d_bitmap", "image", "compression", "jpeg"))
    for parent, dependency in (("javascript", "dom_string"), ("javascript", "custom_element_name"),
                               ("headers_api", "http_token"), ("text_encoding", "text_codec"),
                               ("execution_limits", "stylesheet_source_budget"), ("image_svg", "svg_reference")):
        source_file = src / (parent + ".rs")
        if source_file.is_file() and f"crate::{dependency}::" in source_file.read_text(encoding="utf-8"):
            names.append(dependency)
    modules = []
    for name in names:
        path = src / (name + ".rs")
        if not path.is_file():
            raise FileNotFoundError(path)
        modules.append(f'#[path = {json.dumps(path.as_posix(), ensure_ascii=False)}]\nmod {name};')
    declarations = "\n".join(modules)
    marker = "// Additional probe crate modules"
    if marker in template:
        source = template.replace(marker, declarations)
    else:
        source = template.replace("#[path = JAVASCRIPT_SOURCE]", declarations + "\n#[path = JAVASCRIPT_SOURCE]")
    source = source.replace("JAVASCRIPT_SOURCE", json.dumps(javascript.as_posix(), ensure_ascii=False))
    destination.write_text(source, encoding="utf-8")
    value = engine.partition("pub enum Value {")[2].partition("\n}")[0]
    flags = ["--cfg", "chezcat_string_units"] if "StringUnits(Vec<u16>)" in value else []
    if "BigInt(" in value:
        flags.extend(("--cfg", "chezcat_bigint"))
    return flags
