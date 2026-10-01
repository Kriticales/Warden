// Verificador de contraste WCAG 2.1 para os tokens do Warden.
// Uso: node tools/contraste.mjs   (a partir de design/prototype)
// Lê tokens.css, resolve as três direções e confere os pares que a interface
// realmente usa. Cores com transparência são compostas sobre a superfície
// indicada antes do cálculo. Sai com código 1 se algum par reprovar.
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../tokens.css", import.meta.url), "utf8");

function block(selectorRegex) {
  const m = css.match(selectorRegex);
  if (!m) throw new Error("bloco não encontrado: " + selectorRegex);
  const vars = {};
  for (const v of m[1].matchAll(/--([\w-]+):\s*([^;]+);/g)) vars[v[1]] = v[2].trim();
  return vars;
}
const grafite = block(/:root\[data-direction="grafite"\]\s*\{([\s\S]*?)\n\}/);
const ardosia = { ...grafite, ...block(/:root\[data-direction="ardosia"\]\s*\{([\s\S]*?)\n\}/) };
const calcita = { ...grafite, ...block(/:root\[data-direction="calcita"\]\s*\{([\s\S]*?)\n\}/) };

function parse(c) {
  c = c.trim();
  let m = c.match(/^#([0-9a-f]{6})$/i);
  if (m) return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)).concat(1);
  m = c.match(/^rgba?\(([^)]+)\)$/);
  if (m) {
    const p = m[1].split(",").map((x) => parseFloat(x));
    return [p[0], p[1], p[2], p[3] ?? 1];
  }
  throw new Error("cor não suportada: " + c);
}
function over(fg, bg) {
  const a = fg[3];
  return [0, 1, 2].map((i) => fg[i] * a + bg[i] * (1 - a)).concat(1);
}
function lum([r, g, b]) {
  const f = (v) => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
function ratio(a, b) {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}

// [frente, fundo, mínimo, descrição]. Fundo pode ser "x@y" = x composto sobre y.
const surfaces = ["bg", "surface", "surface-2", "surface-3"];
const pairs = [];
for (const s of surfaces) {
  pairs.push(["c-text", "c-" + s, 4.5, "texto principal"]);
  pairs.push(["c-text-2", "c-" + s, 4.5, "texto secundário"]);
  pairs.push(["c-text-3", "c-" + s, 4.5, "texto apagado"]);
  pairs.push(["c-accent-text", "c-" + s, 4.5, "link/acento como texto"]);
  pairs.push(["c-control-border", "c-" + s, 3, "contorno de campo (1.4.11)"]);
  pairs.push(["c-focus", "c-" + s, 3, "anel de foco (1.4.11)"]);
}
for (const st of ["ok", "warn", "danger", "info"]) {
  for (const s of ["bg", "surface", "surface-2"]) pairs.push([`c-${st}-text`, "c-" + s, 4.5, `texto de estado ${st}`]);
  pairs.push([`c-${st}-text`, `c-${st}-soft@c-surface`, 4.5, `texto ${st} sobre fundo suave`]);
  pairs.push([`c-${st}`, "c-surface", 3, `ícone de estado ${st}`]);
}
pairs.push(["c-text", "c-accent-soft@c-surface", 4.5, "texto em item selecionado"]);
pairs.push(["c-accent-text", "c-accent-soft@c-surface", 4.5, "acento sobre item selecionado"]);
pairs.push(["c-on-accent", "c-accent", 4.5, "texto em botão primário"]);
pairs.push(["c-on-accent", "c-accent-hover", 4.5, "texto em botão primário (hover)"]);
pairs.push(["c-on-danger", "c-danger-solid", 4.5, "texto em botão de perigo"]);
pairs.push(["c-accent", "c-surface", 3, "botão primário contra a superfície"]);
pairs.push(["c-text", "c-diff-add@c-surface", 4.5, "texto em linha adicionada"]);
pairs.push(["c-text", "c-diff-del@c-surface", 4.5, "texto em linha removida"]);
for (const k of ["console-text", "console-muted", "console-warn", "console-error"])
  pairs.push(["c-" + k, "c-console-bg", 4.5, "console: " + k]);
pairs.push(["c-data", "c-data-track", 3, "barra de dados contra o trilho"]);
pairs.push(["c-src-modrinth", "c-surface", 3, "marcador Modrinth"]);
pairs.push(["c-src-curseforge", "c-surface", 3, "marcador CurseForge"]);
pairs.push(["c-src-local", "c-surface", 3, "marcador local"]);

let fails = 0;
for (const [name, t] of Object.entries({ grafite, ardosia, calcita })) {
  console.log(`\n== ${name}`);
  for (const [fg, bgSpec, min, desc] of pairs) {
    let bg;
    if (bgSpec.includes("@")) {
      const [top, base] = bgSpec.split("@");
      bg = over(parse(t[top]), parse(t[base]));
    } else bg = parse(t[bgSpec]);
    const f = over(parse(t[fg]), bg);
    const r = ratio(f, bg);
    const ok = r >= min;
    if (!ok) fails++;
    if (!ok || process.argv.includes("--todos"))
      console.log(`${ok ? "ok  " : "FALHA"} ${r.toFixed(2).padStart(5)} (mín ${min}) ${fg} sobre ${bgSpec} — ${desc}`);
  }
  console.log(`   ${pairs.length} pares conferidos`);
}
console.log(fails ? `\n${fails} par(es) reprovado(s)` : "\nTodos os pares passaram (WCAG 2.1 AA).");
process.exit(fails ? 1 : 0);
