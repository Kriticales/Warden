"""Recorta golden logs curtos de uma captura do protótipo (spike S-R5-3).

A captura tem uma linha por linha de saída do jogo, com o prefixo `[  12.345s out] `.
O recorte junta os registros inteiros (evento XML do log4j ou linha de texto com o
stack trace que vem depois), guarda só os que casam com algum padrão, tira o prefixo
de tempo e troca dados pessoais por marcadores:
  caminho da pasta de dados do spike  -> <DADOS>
  C:\\Users\\<nome>                    -> <PERFIL>
  nome do usuário do Windows           -> <USUARIO>
  UUID do jogador offline              -> <UUID>

Uso: python golden.py <captura.log> <saída.log> <regex> [<regex>...] [--max-stack N]
"""

import os
import re
import sys
from pathlib import Path

PREFIX = re.compile(r"^\[\s*[\d.]+s (?:out|err)\] ?")
NEW_TEXT_RECORD = re.compile(r"^(\[[^\]]+\]|\d{4}-\d\d-\d\d|[A-Z]+ StatusConsoleListener|<log4j:Event|---- Minecraft Crash Report)")
DATA = os.environ.get("WARDEN_SPIKE_DATA", r"C:\wt\s-r5-3\data")
USER = os.environ.get("USERNAME", "") or Path.home().name


def redact(s: str) -> str:
    for variant in (DATA, DATA.replace("\\", "/"), DATA.replace("\\", "\\\\")):
        s = s.replace(variant, "<DADOS>")
    s = re.sub(r"(?i)C:[\\/]+Users[\\/]+[^\\/\s\"']+", "<PERFIL>", s)
    if USER:
        s = re.sub(re.escape(USER), "<USUARIO>", s, flags=re.I)
    s = re.sub(r"\b6c5aa2b1-?c084-?39d7-?99a8-?edc5b372e5f9\b", "<UUID>", s, flags=re.I)
    return s


def records(lines: list[str]) -> list[list[str]]:
    out: list[list[str]] = []
    in_xml = False
    for raw in lines:
        line = PREFIX.sub("", raw.rstrip("\r\n"))
        if in_xml:
            out[-1].append(line)
            if "</log4j:Event>" in line:
                in_xml = False
            continue
        if line.lstrip().startswith("<log4j:Event"):
            out.append([line])
            in_xml = "</log4j:Event>" not in line
        elif not out or NEW_TEXT_RECORD.match(line):
            out.append([line])
        else:
            out[-1].append(line)  # continuação (stack trace, lista de mods...)
    return out


def main() -> None:
    args = sys.argv[1:]
    max_stack = 12
    if "--max-stack" in args:
        i = args.index("--max-stack")
        max_stack = int(args[i + 1])
        del args[i:i + 2]
    src, dst, pats = Path(args[0]), Path(args[1]), [re.compile(p) for p in args[2:]]
    recs = records(src.read_text(encoding="utf-8", errors="replace").splitlines())
    picked = []
    for r in recs:
        text = "\n".join(r)
        if any(p.search(text) for p in pats):
            if len(r) > max_stack + 3:
                # Evento XML: mantém o fechamento (`]]></log4j:Throwable>` e `</log4j:Event>`).
                tail = r[-2:] if r[0].lstrip().startswith("<log4j:Event") else []
                r = r[: max_stack + 1] + ["\t... (cortado no golden log)"] + tail
            picked.append("\n".join(r))
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text(redact("\n".join(picked)) + "\n", encoding="utf-8", newline="\n")
    print(f"{dst}: {len(picked)} registros")


if __name__ == "__main__":
    main()
