// Resume results/*.jsonl (saída do run-case.ps1) numa tabela Markdown.
import { readdirSync, readFileSync } from "node:fs";
const pct = (a, b) => (b === 0 ? (a === 0 ? 0 : Infinity) : ((a - b) / b) * 100);
// O jstat mostra "-" nas colunas que o coletor não tem (ZGC, Shenandoah): conta como 0.
const fixJstat = (r) => ({ ...r, jstat_kb: ["S0U", "S1U", "EU", "OU"].reduce((s, k) => s + (r.jstat[k] ?? 0), 0) });
const rows = [];
for (const f of readdirSync("results").filter((f) => f.endsWith(".jsonl")).sort()) {
  const recs = readFileSync(`results/${f}`, "utf8").split(/\r?\n/).filter((l) => l.startsWith('{"label"')).map((l) => JSON.parse(l.replace(/\bNaN\b/g, "null"))).map(fixJstat);
  const by = {};
  for (const r of recs) (by[r.label.split("/")[1]] ??= []).push(r);
  for (const [step, rs] of Object.entries(by)) {
    const r = rs[rs.length - 1];
    const maxA = Math.max(...rs.map((r) => Math.abs(pct(r.reader_a_kb, r.jstat_kb))));
    const maxB = Math.max(...rs.map((r) => Math.abs(pct(r.reader_b_kb, r.jstat_kb))));
    const same = rs.filter((r) => Math.abs(pct(r.reader_b_kb, r.jstat_kb)) <= 0.5).length;
    const gcOk = rs.every((r) => r.jstat_gc >= r.reader_gc_a && r.jstat_gc <= r.reader_gc_b);
    rows.push(`| ${f.replace(".jsonl", "")} | ${step} | ${rs.length} | ${(r.reader_a_kb / 1024).toFixed(1)} | ${(r.jstat_kb / 1024).toFixed(1)} | ${(r.reader_b_kb / 1024).toFixed(1)} | ${maxA.toFixed(2)}% | ${maxB.toFixed(2)}% | ${same}/${rs.length} | ${r.reader_gc_a}/${r.jstat_gc}/${r.reader_gc_b} ${gcOk ? "ok" : "DIVERGE"} |`);
  }
}
console.log("| caso | etapa | n | leitor antes (MB) | jstat (MB) | leitor depois (MB) | dif. máx. antes | dif. máx. depois | depois = jstat (±0,5%) | coletas antes/jstat/depois |");
console.log("|---|---|---|---|---|---|---|---|---|---|");
console.log(rows.join("\n"));
