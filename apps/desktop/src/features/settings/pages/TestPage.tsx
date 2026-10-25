/**
 * Configurações → Teste (T21): memória padrão, a tabela de Java (L-01, CA-T21-04), versões beta
 * e alpha, a verificação de atualizações e a EULA do Minecraft (L-09, CA-T21-05).
 */
import { useTranslation } from 'react-i18next';

import type { TestMemory } from '../../../lib/ipc/bindings';
import { useSettings } from '../api';
import { EulaSetting } from '../eula/EulaSetting';
import { JavaSettings } from '../java/components/JavaSettings';
import { RowSelect, RowSwitch, SettingRow, SettingsFrame } from '../layout/SettingsFrame';
import { useSaveSetting } from '../layout/save';

/** Valores fixos oferecidos para a memória, em GB. */
const MEMORY_GB = [4, 6, 8, 12, 16] as const;

/** Intervalos oferecidos para a verificação de atualizações, em horas (0 = desligada). */
const INTERVALS = ['6', '24', '168', '0'] as const;

function memoryValue(memory: TestMemory): string {
  return memory.mode === 'auto' ? 'auto' : String(memory.mb);
}

export function TestPage() {
  const { t } = useTranslation('configuracoes');
  const { t: ts } = useTranslation('servidor');
  const settings = useSettings();
  const { save, pending } = useSaveSetting();
  const memory = settings.data?.testMemory ?? { mode: 'auto' };
  const prerelease = settings.data?.showPrereleaseVersions ?? false;
  const interval = String(settings.data?.updateCheckIntervalHours ?? 24);

  const memoryOptions: { value: string; label: string }[] = [
    { value: 'auto', label: t('teste.memoriaAuto') },
    ...MEMORY_GB.map((gb) => ({ value: String(gb * 1024), label: t('teste.memoriaGb', { gb }) })),
  ];
  if (!memoryOptions.some((option) => option.value === memoryValue(memory))) {
    // Valor gravado fora da lista (por outra versão ou à mão): continua visível e escolhido.
    memoryOptions.push({
      value: memoryValue(memory),
      label: t('teste.memoriaMb', { mb: memoryValue(memory) }),
    });
  }

  const intervalOptions: { value: string; label: string }[] = INTERVALS.map((hours) => ({
    value: hours,
    label: t(`teste.intervalo.${hours}`),
  }));
  if (!intervalOptions.some((option) => option.value === interval)) {
    intervalOptions.push({
      value: interval,
      label: t('teste.intervalo.outro', { horas: interval }),
    });
  }

  return (
    <SettingsFrame page="teste">
      <div className="sform">
        <SettingRow title={t('teste.memoria')} desc={t('teste.memoriaDica')}>
          <RowSelect
            value={memoryValue(memory)}
            options={memoryOptions}
            disabled={pending}
            onChange={(value) => {
              save({
                testMemory:
                  value === 'auto' ? { mode: 'auto' } : { mode: 'fixed', mb: Number(value) },
              });
            }}
          />
        </SettingRow>
        <SettingRow title={t('teste.java')} desc={t('teste.javaDesc')} wide stack>
          <JavaSettings bare />
        </SettingRow>
        <SettingRow title={t('teste.beta')} desc={t('teste.betaDesc')}>
          <RowSwitch
            checked={prerelease}
            disabled={pending}
            onChange={(checked) => {
              save({ showPrereleaseVersions: checked });
            }}
          />
        </SettingRow>
        <SettingRow title={t('teste.atualizacoes')} desc={t('teste.atualizacoesDesc')}>
          <RowSelect
            value={interval}
            options={intervalOptions}
            disabled={pending}
            onChange={(value) => {
              save({ updateCheckIntervalHours: Number(value) });
            }}
          />
        </SettingRow>
        <SettingRow title={ts('configuracoes.rotulo')} desc={ts('configuracoes.dica')}>
          <EulaSetting bare />
        </SettingRow>
      </div>
    </SettingsFrame>
  );
}
