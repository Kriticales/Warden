/**
 * Frases dos erros por domínio (ARCHITECTURE §5). Cada domínio tem o seu arquivo, tipado como
 * `Record<CódigoDoDomínio, string>`: o TypeScript falha se faltar uma tradução. Uma frase com
 * número pode ter também `CÓDIGO_one`/`CÓDIGO_other`, como no i18next (`pt-BR/exportar.ts`):
 * com um `count` nos parâmetros, `errorMessage` usa a forma que concorda com ele.
 *
 * Registro acréscimo-apenas (ROADMAP §1): uma linha de import e uma linha em `errorMessages`
 * por domínio.
 */
import type { ErrorCode } from '../../lib/ipc/bindings';
import type { ErrorPhrases } from './plural';

import { app } from './app';
import { core } from './core';
import { packwiz } from './packwiz';
import { packwizCli } from './packwiz-cli';
import { project } from './project';
import { jarmeta } from './jarmeta';
import { modrinth } from './modrinth';
import { curseforge } from './curseforge';
import { catalog } from './catalog';
import { configs } from './configs';
import { java } from './java';
import { launcher } from './launcher';
import { instance } from './instance';
import { diagnostics } from './diagnostics';
import { ai } from './ai';
import { versioning } from './versioning';
import { exportErrors } from './export';
import { secrets } from './secrets';
import { http } from './http';
import { mixin } from './mixin';
import { bisect } from './bisect';
import { server } from './server';
import { perf } from './perf';
import { discovery } from './discovery';
import { importErrors } from './import';
import { scripts } from './scripts';
import { github } from './github';

type ErrorMessages = {
  [Domain in ErrorCode['domain']]: ErrorPhrases<Extract<ErrorCode, { domain: Domain }>['code']>;
};

export const errorMessages: ErrorMessages = {
  app,
  core,
  packwiz,
  packwizCli,
  project,
  jarmeta,
  modrinth,
  curseforge,
  catalog,
  configs,
  java,
  launcher,
  instance,
  diagnostics,
  ai,
  versioning,
  export: exportErrors,
  secrets,
  http,
  mixin,
  bisect,
  server,
  perf,
  discovery,
  import: importErrors,
  scripts,
  github,
};

const pluralRules = new Intl.PluralRules('pt-BR');

/**
 * Frase pt-BR de um código de erro; código desconhecido cai na frase de `app.INTERNAL`. Com
 * `count`, usa `CÓDIGO_one`/`CÓDIGO_other` quando o domínio os tem (a mesma regra de plural do
 * i18next); sem eles, a frase do código.
 */
export function errorMessage(code: ErrorCode, count?: number): string {
  const byCode: Partial<Record<string, string>> = errorMessages[code.domain];
  const plural =
    count === undefined
      ? undefined
      : (byCode[`${code.code}_${pluralRules.select(count)}`] ?? byCode[`${code.code}_other`]);
  return plural ?? byCode[code.code] ?? errorMessages.app.INTERNAL;
}
