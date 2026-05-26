#!/usr/bin/env python3
"""
Medição do `export-json` sem e com `--sysroot`.

Roda o subcomando duas vezes no crate apontado por --path (default: cwd),
salva os dois JSONs e imprime os números pedidos pelo roteiro
(`docs/roteiro-medicao-sysroot.md`) lado a lado.

Não decide nada — só apresenta. A decisão ("filtrar no fork ou na lente")
fica com quem lê os números, conforme o roteiro.

Uso:
    python3 docs/medir.py --path /caminho/do/seu/crate
    python3 docs/medir.py --bin /caminho/cargo-modules --path .

Sem dependências externas: só Python 3 e o binário `cargo-modules` (ou
`cargo modules`) instalado.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any

STDLIB_PREFIXES = ("std", "core", "alloc", "proc_macro", "test")


def is_stdlib(path: str) -> bool:
    return path.split("::", 1)[0] in STDLIB_PREFIXES


def human_bytes(n: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024:
            return f"{n:.1f} {unit}" if unit != "B" else f"{n} B"
        n /= 1024
    return f"{n:.1f} TB"


def run_export(
    bin_path: str,
    crate_path: Path,
    sysroot: bool,
    out_file: Path,
    extra_args: list[str] | None = None,
) -> None:
    args: list[str] = []
    if bin_path == "cargo":
        args = ["cargo", "modules", "export-json", "--compact"]
    else:
        args = [bin_path, "export-json", "--compact"]
    if sysroot:
        args.append("--sysroot")
    if extra_args:
        args.extend(extra_args)

    label = "com sysroot" if sysroot else "sem sysroot"
    print(f"[medir] rodando {label} em {crate_path} ...", file=sys.stderr, flush=True)

    with out_file.open("w") as fh:
        result = subprocess.run(
            args,
            cwd=crate_path,
            stdout=fh,
            stderr=subprocess.PIPE,
            text=True,
        )
    if result.returncode != 0:
        sys.stderr.write(
            f"[medir] FALHA ({label}): exit {result.returncode}\n"
            f"stderr:\n{result.stderr}\n"
        )
        sys.exit(result.returncode)


def measure(json_path: Path) -> dict[str, Any]:
    with json_path.open() as fh:
        data = json.load(fh)
    nodes = data["nodes"]
    edges = data["edges"]
    n_std = [n for n in nodes if is_stdlib(n["path"])]
    n_user = [n for n in nodes if not is_stdlib(n["path"])]
    e_to_std = [e for e in edges if is_stdlib(e["to"])]
    e_to_user = [e for e in edges if not is_stdlib(e["to"])]
    return {
        "file": str(json_path),
        "size_bytes": json_path.stat().st_size,
        "crate": data.get("crate"),
        "nodes_total": len(nodes),
        "nodes_stdlib": len(n_std),
        "nodes_user": len(n_user),
        "edges_total": len(edges),
        "edges_to_stdlib": len(e_to_std),
        "edges_to_user": len(e_to_user),
        "kinds": dict(Counter(n["kind"] for n in nodes)),
        "visibilities": dict(Counter(n["visibility"] for n in nodes)),
        "relations": dict(Counter(e["relation"] for e in edges)),
    }


def pct(part: int, total: int) -> str:
    if total == 0:
        return "  - "
    return f"{100 * part / total:5.1f}%"


def delta(after: int, before: int) -> str:
    diff = after - before
    sign = "+" if diff >= 0 else ""
    return f"{sign}{diff}"


def print_table(without: dict[str, Any], with_: dict[str, Any]) -> None:
    print()
    print(f"crate analisado: {without['crate']!r}")
    print()
    print(f"{'':38} {'sem --sysroot':>15} {'com --sysroot':>15} {'Δ':>10}")
    print("-" * 82)

    def row(label: str, a: int, b: int, total_a: int | None = None, total_b: int | None = None):
        a_str = f"{a}"
        b_str = f"{b}"
        if total_a is not None:
            a_str = f"{a} ({pct(a, total_a)})"
            b_str = f"{b} ({pct(b, total_b or 0)})"
        print(f"{label:38} {a_str:>15} {b_str:>15} {delta(b, a):>10}")

    row("nós totais",          without["nodes_total"],   with_["nodes_total"])
    row("  nós do crate-alvo", without["nodes_user"],    with_["nodes_user"],
        without["nodes_total"], with_["nodes_total"])
    row("  nós da stdlib",     without["nodes_stdlib"],  with_["nodes_stdlib"],
        without["nodes_total"], with_["nodes_total"])
    print()
    row("arestas totais",      without["edges_total"],   with_["edges_total"])
    row("  arestas → crate",   without["edges_to_user"], with_["edges_to_user"],
        without["edges_total"], with_["edges_total"])
    row("  arestas → stdlib",  without["edges_to_stdlib"], with_["edges_to_stdlib"],
        without["edges_total"], with_["edges_total"])
    print()
    print(
        f"{'tamanho do JSON (compact)':38} "
        f"{human_bytes(without['size_bytes']):>15} "
        f"{human_bytes(with_['size_bytes']):>15} "
        f"{delta(with_['size_bytes'], without['size_bytes']):>10}"
    )


def print_breakdown(label: str, snap: dict[str, Any]) -> None:
    print()
    print(f"[{label}] distribuição")
    print(f"  kinds:        {snap['kinds']}")
    print(f"  visibilities: {snap['visibilities']}")
    print(f"  relations:    {snap['relations']}")


def print_reading_guide(without: dict[str, Any], with_: dict[str, Any]) -> None:
    n_total = with_["nodes_total"]
    e_total = with_["edges_total"]
    stdlib_node_pct = 100 * with_["nodes_stdlib"] / n_total if n_total else 0
    stdlib_edge_pct = 100 * with_["edges_to_stdlib"] / e_total if e_total else 0
    node_growth = with_["nodes_total"] - without["nodes_total"]
    edge_growth = with_["edges_total"] - without["edges_total"]
    size_mb = with_["size_bytes"] / (1024 * 1024)

    print()
    print("leitura (use os limiares do roteiro p/ decidir, não confie cegamente):")
    print(f"  • stdlib ocupa {stdlib_node_pct:.1f}% dos nós e {stdlib_edge_pct:.1f}% das arestas (com --sysroot).")
    print(f"  • ligar --sysroot revelou {node_growth} nós e {edge_growth} arestas a mais.")
    print(f"  • JSON com sysroot pesa {size_mb:.2f} MB.")

    flags = []
    if stdlib_node_pct < 20:
        flags.append("stdlib é fração pequena (< 20% dos nós) — filtrar na L1 fica barato.")
    elif stdlib_node_pct > 50:
        flags.append("stdlib domina (> 50% dos nós) — filtrar no fork economiza muito.")
    if size_mb > 10:
        flags.append("JSON > 10 MB — carregar tudo na lente vai pesar; considere filtrar antes.")
    if node_growth == 0 and edge_growth == 0:
        flags.append("nenhum nó/aresta a mais com --sysroot — ou o crate não usa derives, ou algo não expandiu.")
    if flags:
        print("  sinais:")
        for f in flags:
            print(f"    - {f}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--path",
        type=Path,
        default=Path.cwd(),
        help="diretório do crate a medir (default: diretório atual)",
    )
    parser.add_argument(
        "--bin",
        default="cargo-modules",
        help='caminho do binário (default: "cargo-modules" no PATH). Use "cargo" para invocar via plugin.',
    )
    parser.add_argument(
        "--outdir",
        type=Path,
        default=Path("/tmp"),
        help="onde salvar os dois JSONs (default: /tmp)",
    )
    parser.add_argument(
        "extra",
        nargs="*",
        help='args extras passados ao export-json (use "--" para separar). '
             'Ex.: medir.py --path . -- -p meu-pacote --lib',
    )
    args = parser.parse_args()

    if args.bin not in ("cargo",) and not shutil.which(args.bin) and not Path(args.bin).is_file():
        print(f"[medir] não encontrei o binário {args.bin!r} no PATH.", file=sys.stderr)
        print("Compile com `cargo build --release` e aponte --bin para target/release/cargo-modules,", file=sys.stderr)
        print("ou use --bin cargo se já tem o plugin instalado globalmente.", file=sys.stderr)
        return 2

    crate_path = args.path.resolve()
    if not (crate_path / "Cargo.toml").is_file():
        print(f"[medir] {crate_path} não tem Cargo.toml — não parece um crate.", file=sys.stderr)
        return 2

    out_without = args.outdir / "lente_sem_sysroot.json"
    out_with = args.outdir / "lente_com_sysroot.json"

    run_export(args.bin, crate_path, sysroot=False, out_file=out_without, extra_args=args.extra)
    run_export(args.bin, crate_path, sysroot=True, out_file=out_with, extra_args=args.extra)

    without = measure(out_without)
    with_ = measure(out_with)

    print_table(without, with_)
    print_breakdown("sem --sysroot", without)
    print_breakdown("com --sysroot", with_)
    print_reading_guide(without, with_)
    print()
    print(f"JSONs salvos em:\n  {out_without}\n  {out_with}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
