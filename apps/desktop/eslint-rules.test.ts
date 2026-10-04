// @vitest-environment node
// Garante que as regras do ESLint que protegem o padrão do projeto estão ativas
// (QUALITY §2.2; a CI da F0-02 também confere que texto solto em JSX falha).
import { ESLint } from 'eslint';
import { describe, expect, it } from 'vitest';

// Os arquivos de exemplo não existem no disco: o projeto padrão do TypeScript os aceita.
const FIXTURES = ['src/routes/teste-regra.tsx', 'src/routes/teste-regra.ts'];
const eslint = new ESLint({
  cwd: import.meta.dirname,
  overrideConfig: {
    languageOptions: {
      parserOptions: {
        projectService: { allowDefaultProject: FIXTURES, defaultProject: 'tsconfig.app.json' },
      },
    },
  },
});

async function ruleIds(code: string, filePath: string): Promise<string[]> {
  const [result] = await eslint.lintText(code, { filePath });
  return (result?.messages ?? []).map((message) => message.ruleId ?? message.message);
}

describe('regras do ESLint', () => {
  it('texto solto em JSX é erro (i18next/no-literal-string)', async () => {
    const code = 'export function Teste() {\n  return <p>Texto solto</p>;\n}\n';
    expect(await ruleIds(code, 'src/routes/teste-regra.tsx')).toContain(
      'i18next/no-literal-string',
    );
  });

  it('texto em atributo visível também é erro', async () => {
    const code = 'export function Teste() {\n  return <img src="x.png" alt="Logotipo" />;\n}\n';
    expect(await ruleIds(code, 'src/routes/teste-regra.tsx')).toContain(
      'i18next/no-literal-string',
    );
  });

  it('texto vindo do catálogo passa', async () => {
    const code =
      "import { useTranslation } from 'react-i18next';\n\n" +
      'export function Teste() {\n' +
      '  const { t } = useTranslation();\n' +
      '  return <p className="x">{t(\'app.nome\')}</p>;\n' +
      '}\n';
    expect(await ruleIds(code, 'src/routes/teste-regra.tsx')).toEqual([]);
  });

  it('invoke direto fora de src/lib/ipc é erro', async () => {
    const code =
      "import { invoke } from '@tauri-apps/api/core';\n\n" +
      "export async function x(): Promise<unknown> {\n  return invoke('app_info');\n}\n";
    expect(await ruleIds(code, 'src/routes/teste-regra.ts')).toContain('no-restricted-imports');
  });
});
