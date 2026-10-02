// Gera os arquivos de tema a partir da fonte única design/system/tokens.json.
// Uso (da raiz do repositório): node design/system/tools/build-tokens.mjs
// Saídas (não edite à mão; o cabeçalho de cada uma avisa):
//   design/system/tokens.css         variáveis CSS para HTML/CSS puro (galeria e protótipo)
//   design/system/tailwind-theme.css bloco @theme do Tailwind CSS 4
//   design/system/shadcn-theme.css   variáveis que os componentes do shadcn/ui esperam
// Sem dependências: só Node 20+.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const tokens = JSON.parse(readFileSync(join(root, "tokens.json"), "utf8"));

const isToken = (o) => o && typeof o === "object" && "$value" in o;
const entries = (group) => Object.entries(tokens[group]).filter(([k, v]) => !k.startsWith("$") && isToken(v));

function resolve(value, seen = new Set()) {
  if (typeof value !== "string") return value;
  const m = value.match(/^\{([\w-]+)\.([\w-]+)\}$/);
  if (!m) return value;
  const key = `${m[1]}.${m[2]}`;
  if (seen.has(key)) throw new Error("referência circular: " + key);
  seen.add(key);
  const t = tokens[m[1]]?.[m[2]];
  if (!isToken(t)) throw new Error("referência inexistente: " + key);
  return resolve(t.$value, seen);
}

function css(t) {
  const v = resolve(t.$value);
  if (t.$type === "fontFamily") return v.map((f) => (/\s/.test(f) ? `"${f}"` : f)).join(", ");
  if (t.$type === "cubicBezier") return `cubic-bezier(${v.join(", ")})`;
  return String(v);
}

const banner = (what) =>
  `/* ARQUIVO GERADO por design/system/tools/build-tokens.mjs a partir de tokens.json.\n   Não edite à mão: mude tokens.json e rode o script de novo.\n   ${what} */\n`;

// ---------- Nome de saída de cada grupo ----------
const out = []; // [cssName, value, group, key]
for (const [k, t] of entries("color")) out.push([`--color-${k}`, css(t), "color", k]);
for (const [k, t] of entries("font")) out.push([`--font-${k}`, css(t), "font", k]);
for (const [k, t] of entries("text")) {
  out.push([`--text-${k}`, css(t), "text", k]);
  out.push([`--text-${k}--line-height`, t.lineHeight, "text", k]);
}
for (const [k, t] of entries("font-weight")) out.push([`--font-weight-${k}`, css(t), "font-weight", k]);
for (const [k, t] of entries("tracking")) out.push([`--tracking-${k}`, css(t), "tracking", k]);
for (const [k, t] of entries("space")) out.push([k === "base" ? "--spacing" : `--space-${k}`, css(t), "space", k]);
for (const [k, t] of entries("radius")) out.push([`--radius-${k}`, css(t), "radius", k]);
for (const [k, t] of entries("notch")) out.push([`--notch-${k}`, css(t), "notch", k]);
for (const [k, t] of entries("border-width")) out.push([`--border-${k}`, css(t), "border-width", k]);
for (const [k, t] of entries("shadow")) out.push([`--${t.inset ? "inset-shadow" : "shadow"}-${k}`, css(t), "shadow", k]);
for (const [k, t] of entries("duration")) out.push([`--duration-${k}`, css(t), "duration", k]);
for (const [k, t] of entries("ease")) out.push([`--ease-${k}`, css(t), "ease", k]);
for (const [k, t] of entries("size")) out.push([`--size-${k}`, css(t), "size", k]);
for (const [k, t] of entries("z")) out.push([`--z-${k}`, css(t), "z", k]);

// Formas derivadas (não são valores livres: dependem do degrau).
const notchPoly = (n) =>
  `polygon(0 ${n}, ${n} ${n}, ${n} 0, calc(100% - ${n}) 0, calc(100% - ${n}) ${n}, 100% ${n}, 100% calc(100% - ${n}), calc(100% - ${n}) calc(100% - ${n}), calc(100% - ${n}) 100%, ${n} 100%, ${n} calc(100% - ${n}), 0 calc(100% - ${n}))`;
const derived = [
  ["--shape-notch-md", notchPoly("var(--notch-md)")],
  ["--shape-notch-sm", notchPoly("var(--notch-sm)")],
];

// ---------- tokens.css ----------
let groups = "";
let last = "";
for (const [name, value, group] of out) {
  if (group !== last) {
    groups += `\n  /* ${group} */\n`;
    last = group;
  }
  groups += `  ${name}: ${value};\n`;
}
const tokensCss =
  banner("Uso: <link rel=\"stylesheet\" href=\"tokens.css\"> antes de components.css.") +
  `:root {\n  color-scheme: dark;\n${groups}\n  /* formas derivadas */\n${derived.map(([n, v]) => `  ${n}: ${v};`).join("\n")}\n}\n\n` +
  `/* Movimento reduzido: tudo instantâneo e sem pulso (o app também oferece a opção em Configurações). */\n` +
  `@media (prefers-reduced-motion: reduce) {\n  :root {\n    --duration-instant: 0ms;\n    --duration-fast: 0ms;\n    --duration-base: 0ms;\n    --duration-slow: 0ms;\n    --duration-pulse: 0ms;\n  }\n}\n` +
  `:root[data-motion="reduced"] {\n  --duration-instant: 0ms;\n  --duration-fast: 0ms;\n  --duration-base: 0ms;\n  --duration-slow: 0ms;\n  --duration-pulse: 0ms;\n}\n`;
writeFileSync(join(root, "tokens.css"), tokensCss);

// ---------- tailwind-theme.css ----------
// Tailwind 4: os namespaces --color-*, --font-*, --text-*, --font-weight-*, --tracking-*,
// --spacing, --radius-*, --shadow-*, --inset-shadow-*, --ease-* viram utilitários.
// Os outros (--notch-*, --duration-*, --size-*, --z-*) ficam como variáveis comuns.
const twNamespaces = ["--color-", "--font-", "--text-", "--tracking-", "--spacing", "--radius-", "--shadow-", "--inset-shadow-", "--ease-"];
const tw = out.filter(([n]) => twNamespaces.some((p) => n.startsWith(p)));
const plain = out.filter(([n]) => !twNamespaces.some((p) => n.startsWith(p)));
const twCss =
  banner("Uso no frontend: @import \"tailwindcss\"; @import \"./tailwind-theme.css\";") +
  `/* Zera as paletas padrão do Tailwind: só existem as cores, sombras e raios do Warden. */\n` +
  `@theme {\n  --color-*: initial;\n  --shadow-*: initial;\n  --inset-shadow-*: initial;\n  --radius-*: initial;\n  --font-*: initial;\n  --text-*: initial;\n\n` +
  tw.map(([n, v]) => `  ${n}: ${v};`).join("\n") +
  `\n\n  --animate-sensor: sensor var(--duration-pulse) var(--ease-in-out) infinite;\n` +
  `  @keyframes sensor {\n    0%, 100% { filter: drop-shadow(0 0 4px rgb(25 211 224 / 0.3)); }\n    50% { filter: drop-shadow(0 0 16px rgb(25 211 224 / 0.7)); }\n  }\n}\n\n` +
  `:root {\n${plain.map(([n, v]) => `  ${n}: ${v};`).join("\n")}\n${derived.map(([n, v]) => `  ${n}: ${v};`).join("\n")}\n}\n\n` +
  `@media (prefers-reduced-motion: reduce) {\n  :root { --duration-instant: 0ms; --duration-fast: 0ms; --duration-base: 0ms; --duration-slow: 0ms; --duration-pulse: 0ms; }\n}\n`;
writeFileSync(join(root, "tailwind-theme.css"), twCss);

// ---------- shadcn-theme.css ----------
// O shadcn/ui espera estas variáveis. Os valores são os do Warden, resolvidos.
// Atenção: no shadcn, "accent" é o fundo de hover (aqui = surface-3), NÃO o ciano.
const c = (k) => css(tokens.color[k]);
const shadcn = {
  background: c("bg"),
  foreground: c("text"),
  card: c("surface-1"),
  "card-foreground": c("text"),
  popover: c("bg-sunken"),
  "popover-foreground": c("text"),
  primary: c("primary"),
  "primary-foreground": c("on-primary"),
  secondary: c("surface-2"),
  "secondary-foreground": c("text"),
  muted: c("surface-2"),
  "muted-foreground": c("text-3"),
  accent: c("surface-3"),
  "accent-foreground": c("text"),
  destructive: c("danger-solid"),
  "destructive-foreground": c("on-danger"),
  border: c("border"),
  input: c("border-control"),
  ring: c("focus"),
  radius: css(tokens.radius.none),
};
const mapped = Object.keys(shadcn).filter((k) => k !== "radius" && !["primary", "border"].includes(k));
const shadcnCss =
  banner("Uso: importe depois de tailwind-theme.css. Mapeia os nomes do shadcn/ui para os tokens do Warden.") +
  `:root {\n${Object.entries(shadcn).map(([k, v]) => `  --${k}: ${v};`).join("\n")}\n}\n\n` +
  `/* primary e border já existem em tailwind-theme.css com o mesmo valor. */\n` +
  `@theme inline {\n${mapped.map((k) => `  --color-${k}: var(--${k});`).join("\n")}\n}\n`;
writeFileSync(join(root, "shadcn-theme.css"), shadcnCss);

console.log(`tokens.css: ${out.length + derived.length} variáveis · tailwind-theme.css: ${tw.length} no @theme · shadcn-theme.css: ${Object.keys(shadcn).length}`);
