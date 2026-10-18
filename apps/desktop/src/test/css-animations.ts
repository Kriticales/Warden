/**
 * Animações de CSS do app no jsdom, para testar o que depende delas (A-01).
 *
 * O jsdom não aplica o CSS do app e não lê o atalho `animation`: para ele, nada anima. Mas o
 * Radix (Presence) decide se um menu ou diálogo que fecha sai da tela na hora ou espera o fim de
 * uma animação pelo `animation-name` calculado. Aqui o `getComputedStyle` passa a devolver o
 * `animation-name` que o navegador calcularia com o CSS de verdade (`design/system/components.css`
 * e `styles/warden.css`, na ordem do `app.css`), e nenhum evento de animação chega, como no
 * WebView2 do runner do Windows, onde o menu fechado ficava preso na página.
 *
 * A cascata é a mínima para isso: estilo inline (`animation` do Popper) e, nas folhas, a regra
 * de maior especificidade (a última, no empate). Camadas e `!important` ficam de fora: os dois
 * arquivos estão na mesma camada e nenhum usa `!important` em `animation`.
 */
import { vi } from 'vitest';

/** A pasta deste arquivo (o Vitest define `import.meta.dirname`; os tipos do Vite não o têm). */
const HERE = (import.meta as ImportMeta & { dirname: string }).dirname;

const STYLESHEETS = [
  `${HERE}/../../../../design/system/components.css`,
  `${HERE}/../styles/warden.css`,
];

/**
 * Lê um arquivo do repositório. O `?raw` do Vite devolve CSS vazio no Vitest, e o tsconfig do app
 * não tem os tipos do Node; o módulo `node:fs` vem por import dinâmico, com o tipo do que se usa.
 */
async function readText(path: string): Promise<string> {
  const specifier = 'node:fs';
  const fs = (await import(/* @vite-ignore */ specifier)) as {
    readFileSync: (file: string, encoding: 'utf8') => string;
  };
  return fs.readFileSync(path, 'utf8');
}

/** Especificidade como número (ids, classes/atributos/pseudoclasses, tipos). */
function specificity(selector: string): number {
  const ids = (selector.match(/#[\w-]+/g) ?? []).length;
  const classes = (selector.match(/\.[\w-]+|\[[^\]]+\]|:(?!:)[\w-]+/g) ?? []).length;
  const types = (
    selector
      .replace(/\[[^\]]+\]|\.[\w-]+|#[\w-]+|::?[\w-]+(\([^)]*\))?/g, ' ')
      .match(/[a-z][\w-]*/gi) ?? []
  ).length;
  return ids * 10_000 + classes * 100 + types;
}

/** O nome no atalho `animation` ("menu-slide var(--duration-base) …" → "menu-slide"). */
function nameOf(animation: string): string {
  return animation.trim().split(/\s+/)[0] ?? 'none';
}

function animationName(element: Element): string {
  if (element instanceof HTMLElement && element.style.animation) {
    return nameOf(element.style.animation);
  }
  let best = { specificity: -1, name: 'none' };
  for (const sheet of Array.from(document.styleSheets)) {
    for (const rule of Array.from(sheet.cssRules)) {
      if (!(rule instanceof CSSStyleRule)) continue;
      const declared = /(?:^|[;{\s])animation:\s*([^;}]+)/.exec(rule.cssText)?.[1];
      if (declared === undefined) continue;
      for (const selector of rule.selectorText.split(',')) {
        let matches: boolean;
        try {
          matches = element.matches(selector);
        } catch {
          // Seletor que o jsdom não entende (`:has`, por exemplo): não é de menu nem de diálogo.
          matches = false;
        }
        const value = specificity(selector);
        if (matches && value >= best.specificity)
          best = { specificity: value, name: nameOf(declared) };
      }
    }
  }
  return best.name;
}

/**
 * Põe o CSS do app na página e faz o `getComputedStyle` devolver o `animation-name` dele. Devolve
 * a função que desfaz as duas coisas (chamar no fim do teste).
 */
export async function applyAppAnimations(): Promise<() => void> {
  const styles: HTMLStyleElement[] = [];
  for (const path of STYLESHEETS) {
    const style = document.createElement('style');
    style.textContent = await readText(path);
    document.head.append(style);
    styles.push(style);
  }
  const realGetComputedStyle = window.getComputedStyle.bind(window);
  const spy = vi.spyOn(window, 'getComputedStyle').mockImplementation((element, pseudo) => {
    const computed = realGetComputedStyle(element, pseudo);
    return new Proxy(computed, {
      get: (target, property) => {
        if (property === 'animationName') return animationName(element);
        const value: unknown = Reflect.get(target, property, target);
        return typeof value === 'function' ? (value as () => unknown).bind(target) : value;
      },
    });
  });
  return () => {
    spy.mockRestore();
    for (const style of styles) style.remove();
  };
}
