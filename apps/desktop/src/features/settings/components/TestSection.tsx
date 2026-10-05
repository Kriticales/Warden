/**
 * Teste (T21): memória padrão, a tabela de Java (encaixe da L-01), versões beta e alpha e a
 * verificação automática de atualizações. A EULA do Minecraft (P1, CA-T21-05) entra com o
 * servidor local.
 */
import { useTranslation } from 'react-i18next';

import type { TestMemory } from '../../../lib/ipc/bindings';
import { useSettings } from '../api';
import { testSectionSlots } from '../slots';
import { SelectField, Switch } from './fields';
import { SettingsPanel, useSaveSetting } from './SettingsPanel';

/** Valores fixos oferecidos para a memória, em GB. */
const MEMORY_GB = [4, 6, 8, 12, 16] as const;

/** Intervalos oferecidos para a verificação de atualizações, em horas (0 = desligada). */
const INTERVALS = ['6', '24', '168', '0'] as const;

function memoryValue(memory: TestMemory): string {
  return memory.mode === 'auto' ? 'auto' : String(memory.mb);
}

export function TestSection() {
  const { t } = useTranslation('configuracoes');
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
    <SettingsPanel title={t('teste.titulo')}>
      <SelectField
        label={t('teste.memoria')}
        hint={t('teste.memoriaDica')}
        options={memoryOptions}
        value={memoryValue(memory)}
        disabled={pending}
        onChange={(event) => {
          const value = event.target.value;
          save({
            testMemory: value === 'auto' ? { mode: 'auto' } : { mode: 'fixed', mb: Number(value) },
          });
        }}
      />
      {testSectionSlots.map((Slot, index) => (
        // Encaixes fixos, registrados no carregamento do módulo.
        <Slot key={index} />
      ))}
      <Switch
        label={t('teste.beta')}
        checked={prerelease}
        disabled={pending}
        onText={t('teste.ligado')}
        offText={t('teste.desligado')}
        onChange={(checked) => {
          save({ showPrereleaseVersions: checked });
        }}
      />
      <SelectField
        label={t('teste.atualizacoes')}
        options={intervalOptions}
        value={interval}
        disabled={pending}
        onChange={(event) => {
          save({ updateCheckIntervalHours: Number(event.target.value) });
        }}
      />
    </SettingsPanel>
  );
}
