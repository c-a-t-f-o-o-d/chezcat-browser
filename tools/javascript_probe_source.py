"""Render a standalone Rust probe against the selected ChezCat source revision."""
import json


def write_probe(template, source_root, destination):
    src = source_root.resolve() / "src"
    javascript = src / "javascript.rs"
    engine = javascript.read_text(encoding="utf-8")
    modules = []
    if "crate::canvas_2d_bitmap::" in engine:
        for name in ("canvas_2d_bitmap", "image", "compression", "jpeg"):
            path = src / (name + ".rs")
            if not path.is_file():
                raise FileNotFoundError(path)
            modules.append(f'#[path = {json.dumps(path.as_posix(), ensure_ascii=False)}]\nmod {name};')
    source = template.replace("// Additional probe crate modules", "\n".join(modules))
    source = source.replace("JAVASCRIPT_SOURCE", json.dumps(javascript.as_posix(), ensure_ascii=False))
    destination.write_text(source, encoding="utf-8")
    value = engine.partition("pub enum Value {")[2].partition("\n}")[0]
    return ["--cfg", "chezcat_string_units"] if "StringUnits(Vec<u16>)" in value else []
