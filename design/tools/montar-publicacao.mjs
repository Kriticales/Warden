// Monta as pastas publicadas como páginas do Superset (design/_publicado/, fora do git).
// Uso (da raiz do repositório): node design/tools/montar-publicacao.mjs
// O protótipo importa ../system/*; a página publicada precisa desses arquivos dentro dela,
// então eles são copiados para system/ e os caminhos do index.html são reescritos.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const design = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(design, "_publicado");
const SYSTEM_FILES = ["tokens.css", "components.css", "components.js", "behavior.js", "icons.js", "contraste.js", "gallery.js", "index.html"];
rmSync(out, { recursive: true, force: true });

function copySystem(dest, withGallery) {
  mkdirSync(join(dest, "fonts"), { recursive: true });
  for (const f of SYSTEM_FILES) if (withGallery || !["gallery.js", "index.html", "contraste.js"].includes(f)) cpSync(join(design, "system", f), join(dest, f));
  cpSync(join(design, "system", "fonts"), join(dest, "fonts"), { recursive: true });
}

// 1. Galeria do design system
const ds = join(out, "design-system");
copySystem(ds, true);

// 2. Protótipo final
const pf = join(out, "prototipo-final");
mkdirSync(pf, { recursive: true });
for (const f of ["app.js", "data.js", "prototipo.css", "screens-app.js", "screens-pack.js", "screens-discover.js", "screens-diag.js", "screens-test.js", "screens-pro-problemas.js", "screens-pro-outros.js"]) cpSync(join(design, "prototipo-final", f), join(pf, f));
copySystem(join(pf, "system"), false);
writeFileSync(join(pf, "index.html"), readFileSync(join(design, "prototipo-final", "index.html"), "utf8").replaceAll("../system/", "system/"));

console.log("Pronto: " + out);
