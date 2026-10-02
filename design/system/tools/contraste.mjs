// Verificador de contraste WCAG 2.1 dos tokens do Warden.
// Uso (da raiz do repositório): node design/system/tools/contraste.mjs
// Lê tokens.json, compõe as cores translúcidas sobre o fundo indicado e confere
// cada par que a interface realmente usa. Escreve design/system/contraste.md e
// sai com código 1 se algum par reprovar. Também gera contraste.js para a galeria.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const tokens = JSON.parse(readFileSync(join(root, "tokens.json"), "utf8"));

function resolve(v) {
  const m = typeof v === "string" && v.match(/^\{([\w-]+)\.([\w-]+)\}$/);
  return m ? resolve(tokens[m[1]][m[2]].$value) : v;
}
const color = (k) => resolve(tokens.color[k].$value);

function parse(c) {
  let m = c.match(/^#([0-9a-f]{6})$/i);
  if (m) return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)).concat(1);
  m = c.match(/^rgba?\(([^)]+)\)$/);
  if (m) {
    const p = m[1].split(",").map((x) => parseFloat(x));
    return [p[0], p[1], p[2], p[3] ?? 1];
  }
  throw new Error("cor não suportada: " + c);
}
const over = (fg, bg) => [0, 1, 2].map((i) => fg[i] * fg[3] + bg[i] * (1 - fg[3])).concat(1);
// "a@b@c" = a composto sobre b, que foi composto sobre c.
function rgb(spec) {
  const parts = spec.split("@").map((k) => parse(color(k)));
  let acc = parts.pop();
  while (parts.length) acc = over(parts.pop(), acc);
  return acc;
}
function lum([r, g, b]) {
  const f = (v) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

// [frente, fundo, mínimo, uso]
const T = 4.5; // texto normal (1.4.3)
const G = 3; // componentes e gráficos (1.4.11)
const surfaces = ["bg", "bg-sunken", "surface-1", "surface-2", "surface-3"];
const pairs = [];
for (const s of surfaces) {
  pairs.push(["text", s, T, "Texto principal"]);
  pairs.push(["text-2", s, T, "Texto secundário"]);
  pairs.push(["text-3", s, T, "Texto apagado / placeholder"]);
  pairs.push(["primary-text", s, T, "Link e texto em ciano"]);
  pairs.push(["bone", s, T, "Texto em osso / IA"]);
  pairs.push(["border-control", s, G, "Contorno de campo"]);
  pairs.push(["focus", s, G, "Anel de foco"]);
  pairs.push(["primary", s, G, "Ciano como marcador (barra, progresso, borda)"]);
}
for (const st of ["ok", "warn", "danger", "info"]) {
  for (const s of ["bg", "surface-1", "surface-2", "surface-3"]) pairs.push([`${st}-text`, s, T, `Texto de estado ${st}`]);
  pairs.push([`${st}-text`, `${st}-soft@surface-1`, T, `Texto ${st} sobre o fundo suave do alerta`]);
  pairs.push(["text", `${st}-soft@surface-1`, T, `Texto principal dentro do alerta ${st}`]);
  pairs.push([st, "surface-1", G, `Ícone/barra ${st}`]);
}
pairs.push(["on-primary", "primary", T, "Texto do botão primário e do Testar"]);
pairs.push(["on-primary", "primary-hover", T, "Botão primário com o mouse em cima"]);
pairs.push(["on-primary", "primary-press", T, "Botão primário pressionado"]);
pairs.push(["on-danger", "danger-solid", T, "Botão de perigo"]);
pairs.push(["text", "primary-soft@surface-1", T, "Texto de item selecionado"]);
pairs.push(["text-3", "primary-soft@surface-1", T, "Metadado de item selecionado"]);
pairs.push(["primary-text", "primary-soft@surface-1", T, "Ciano sobre item selecionado"]);
pairs.push(["text", "ai-soft@surface-1", T, "Texto do bloco de resposta da IA"]);
pairs.push(["ai", "ai-soft@surface-1", T, "Rótulo em osso no bloco da IA"]);
pairs.push(["text", "diff-add@bg-sunken", T, "Linha adicionada no diff"]);
pairs.push(["text", "diff-del@bg-sunken", T, "Linha removida no diff"]);
pairs.push(["ok-text", "diff-add@bg-sunken", T, "Sinal + no diff"]);
pairs.push(["danger-text", "diff-del@bg-sunken", T, "Sinal − no diff"]);
pairs.push(["console-text", "console-bg", T, "Console: texto"]);
pairs.push(["console-muted", "console-bg", T, "Console: hora e origem"]);
pairs.push(["warn-text", "console-bg", T, "Console: aviso"]);
pairs.push(["danger-text", "console-bg", T, "Console: erro"]);
pairs.push(["src-modrinth", "surface-1", G, "Marcador Modrinth"]);
pairs.push(["src-curseforge", "surface-1", G, "Marcador CurseForge"]);
pairs.push(["src-local", "surface-1", G, "Marcador arquivo local"]);
// D4: funções avançadas
pairs.push(["ok-text", "bg-sunken", T, "Chip de evidência conferida; string no editor de scripts"]);
pairs.push(["ok", "bg-sunken", G, "Contorno do chip de evidência"]);
pairs.push(["info-text", "bg-sunken", T, "Palavra-chave no editor de scripts"]);
pairs.push(["text", "ai-soft@bg", T, "Texto da mensagem da IA na conversa"]);
pairs.push(["text-2", "ai-soft@bg", T, "Texto secundário na mensagem da IA"]);
pairs.push(["ai", "ai-soft@bg", T, "Rótulo em osso na mensagem da IA"]);
pairs.push(["text", "danger-soft@console-bg", T, "Console agrupado: linha de erro"]);
pairs.push(["danger-text", "danger-soft@console-bg", T, "Console agrupado: nível de erro"]);
pairs.push(["text", "warn-soft@console-bg", T, "Stack trace: primeira linha de mod"]);
pairs.push(["warn", "surface-3", G, "Faixa de suspeitos e blocos de memória alta"]);
pairs.push(["danger", "danger-soft@surface-2", G, "Contorno da rodada que travou"]);
pairs.push(["ok", "ok-soft@surface-2", G, "Contorno da rodada que passou"]);
pairs.push(["warn-text", "warn-soft@surface-1", T, "Memória alta na faixa de desempenho"]);
pairs.push(["primary", "surface-3", G, "Blocos do mini-gráfico de memória"]);
pairs.push(["text", "overlay@surface-1", T, "Texto atrás do diálogo (não precisa, só referência)"]);

let fails = 0;
const data = [];
const rows = pairs.map(([fg, bg, min, use]) => {
  const r = ratio(rgb(fg), rgb(bg));
  data.push({ fg, bg, ratio: +r.toFixed(2), min, use });
  const ok = r >= min;
  if (!ok) fails++;
  const lvl = min === G ? (ok ? "AA (3:1)" : "REPROVA") : r >= 7 ? "AAA" : ok ? "AA" : "REPROVA";
  return `| \`${fg}\` | \`${bg.replace(/@/g, " sobre ")}\` | ${r.toFixed(2)}:1 | ${min}:1 | ${lvl} | ${use} |`;
});
// a linha de referência do overlay não conta
const md =
  `# Contraste dos tokens (WCAG 2.1)\n\nGerado por \`node design/system/tools/contraste.mjs\` a partir de \`tokens.json\`. Cores translúcidas são compostas sobre o fundo indicado antes do cálculo. Mínimo 4,5:1 para texto (1.4.3) e 3:1 para contornos, ícones e marcadores (1.4.11).\n\n` +
  `| Frente | Fundo | Contraste | Mínimo | Nível | Uso |\n|---|---|---|---|---|---|\n${rows.slice(0, -1).join("\n")}\n\n` +
  `**${pairs.length - 1} pares conferidos, ${fails ? fails + " reprovados" : "todos aprovados"}.**\n`;
writeFileSync(join(root, "contraste.md"), md);
writeFileSync(join(root, "contraste.js"), "// ARQUIVO GERADO por tools/contraste.mjs (dados para a galeria)\nwindow.WARDEN_CONTRAST = " + JSON.stringify(data.slice(0, -1)) + ";\n");
console.log(md.split("\n").filter((l) => l.includes("REPROVA")).join("\n") || "nenhuma reprovação");
console.log(`${pairs.length - 1} pares, ${fails} reprovados`);
process.exit(fails ? 1 : 0);
