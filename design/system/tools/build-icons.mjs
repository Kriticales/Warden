// Gera design/system/icons.js com o subconjunto de ícones Lucide (ISC) usado pelo Warden.
// Uso: node design/system/tools/build-icons.mjs <pasta icons do pacote lucide-static>
// (ex.: npm i lucide-static numa pasta temporária). No app React os mesmos nomes vêm de lucide-react.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const dir = process.argv[2];
if (!dir) throw new Error("informe a pasta icons do lucide-static");
const NAMES = `arrow-left arrow-right settings play chevron-down chevron-right chevron-up search link folder-open plus check x
triangle-alert circle-alert circle-x circle-help info circle-check refresh-cw trash-2 pencil ellipsis monitor server package
file-text folder history cloud-upload download copy external-link square terminal sparkles tag clock memory-stick coffee
key-round shield lock eye eye-off list-filter arrow-up-down pause save undo-2 wrench puzzle image sun file-diff
git-compare globe inbox flag book-open list minus archive file-code hard-drive box layers file-plus rotate-ccw
scroll-text cpu user
network activity gauge messages-square send target flask-conical import images braces circle-dot circle-dashed
circle-minus timer heart-pulse scan-search list-tree chart-column text-search hand sliders-horizontal file-archive code
square-terminal upload check-check`.split(/\s+/);
const version = readFileSync(join(dir, "..", "package.json"), "utf8").match(/"version":\s*"([^"]+)"/)[1];
const icons = {};
for (const n of NAMES) {
  const svg = readFileSync(join(dir, n + ".svg"), "utf8");
  icons[n] = svg.slice(svg.indexOf(">", svg.indexOf("<svg")) + 1, svg.lastIndexOf("</svg>")).replace(/\s*\n\s*/g, "").trim();
}
const out = `// ARQUIVO GERADO por design/system/tools/build-icons.mjs. Não edite à mão.
// Ícones Lucide v${version}, licença ISC (c) Lucide Icons and Contributors. https://lucide.dev
// No app React use lucide-react com os mesmos nomes (ex.: "triangle-alert" -> <TriangleAlert />).
window.WARDEN_ICONS = ${JSON.stringify(icons, null, 0).replace(/","/g, '",\n  "')};
`;
writeFileSync(join(dirname(fileURLToPath(import.meta.url)), "..", "icons.js"), out);
console.log(NAMES.length + " ícones");
