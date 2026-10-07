/**
 * "Mais opções" do painel de detalhes do item (SPEC T07 e T11; protótipo `detailDrawer`):
 * **Fixar versão** (`pin = true`) e **Opcional para o jogador** (tabela `[option]` com
 * descrição e "ligado por padrão"). Cada mudança grava o `.pw.toml` na hora, pela mesma
 * transação da lista; o aviso fixo diz o que se perde nos outros formatos.
 */
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { OptionSettings } from '../../../lib/ipc/bindings';
import { Switch } from '../../settings/components/fields';
import type { DetailBlockProps } from '../details/blocks';
import { useItemOption, useSetOptional, useSetPinned } from './api';

/** Tamanho máximo da descrição (o mesmo da `warden-project`). */
export const DESCRIPTION_MAX = 300;

/** O padrão ao marcar um item como opcional: vem ligado, como o packwiz-installer sugere. */
const NEW_OPTION: OptionSettings = { description: '', default: true };

export function ItemOptionsBlock({ packId, item }: DetailBlockProps) {
  const { t } = useTranslation('opcionais');
  // Só metafile válido tem `pin` e `[option]` (arquivo local ou inválido: nada a editar).
  const editable = item.state === 'ok' && item.sideEditable;
  const option = useItemOption(packId, item.path, editable);
  const setPinned = useSetPinned(packId);
  const setOptional = useSetOptional(packId, item.path);
  if (!editable) return null;

  const settings = option.data ?? null;
  return (
    <details className="disclosure" data-testid="mais-opcoes">
      <summary>{t('maisOpcoes')}</summary>
      <div className="stack-3" style={{ marginTop: 8 }}>
        <div className="stack-1">
          <Switch
            label={t('fixar.rotulo')}
            checked={item.pinned}
            disabled={setPinned.isPending}
            onText={t('ligado')}
            offText={t('desligado')}
            onChange={(pinned) => {
              setPinned.mutate(
                { paths: [item.path], pinned },
                {
                  onSuccess: () => {
                    showToast({
                      kind: 'ok',
                      title: pinned ? t('fixar.fixado') : t('fixar.solto'),
                    });
                  },
                },
              );
            }}
          />
          <div className="field__hint">{t('fixar.dica')}</div>
          {setPinned.isError ? <ErrorPanel compact error={setPinned.error} /> : null}
        </div>

        <div className="stack-1">
          <Switch
            label={t('opcional.rotulo')}
            checked={settings !== null}
            disabled={option.isPending || setOptional.isPending}
            onText={t('ligado')}
            offText={t('desligado')}
            onChange={(on) => {
              setOptional.mutate(on ? NEW_OPTION : null, {
                onSuccess: () => {
                  showToast({
                    kind: 'ok',
                    title: on ? t('opcional.marcado') : t('opcional.desmarcado'),
                  });
                },
              });
            }}
          />
          <div className="field__hint">{t('opcional.dica')}</div>
          {option.isError ? <ErrorPanel compact error={option.error} /> : null}
        </div>

        {settings ? (
          // A chave refaz o formulário quando o disco muda (depois de salvar, por exemplo).
          <OptionForm
            key={JSON.stringify(settings)}
            settings={settings}
            saving={setOptional.isPending}
            error={setOptional.isError ? setOptional.error : undefined}
            onSave={(next) => {
              setOptional.mutate(next, {
                onSuccess: () => {
                  showToast({ kind: 'ok', title: t('opcional.salvo') });
                },
              });
            }}
          />
        ) : null}

        <Alert kind="warn" compact title={t('avisoFormatosTitulo')}>
          <div className="alert__text">{t('avisoFormatos')}</div>
        </Alert>
      </div>
    </details>
  );
}

function OptionForm({
  settings,
  saving,
  error,
  onSave,
}: {
  settings: OptionSettings;
  saving: boolean;
  error: unknown;
  onSave: (settings: OptionSettings) => void;
}) {
  const { t } = useTranslation('opcionais');
  const id = useId();
  const [description, setDescription] = useState(settings.description);
  const [byDefault, setByDefault] = useState(settings.default);
  const trimmed = description.trim();
  const problem =
    Array.from(trimmed).length > DESCRIPTION_MAX
      ? t('opcional.muitoLongo', { max: DESCRIPTION_MAX })
      : /[\r\n\t]/.test(trimmed)
        ? t('opcional.umaLinha')
        : null;
  const changed = trimmed !== settings.description || byDefault !== settings.default;
  return (
    <form
      className="stack-3"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        if (problem === null && changed) onSave({ description: trimmed, default: byDefault });
      }}
    >
      <div className="field">
        <label className="field__label" htmlFor={id}>
          {t('opcional.descricao')} <span className="opt">{t('opcional.descricaoOpcional')}</span>
        </label>
        <input
          id={id}
          className="input"
          type="text"
          value={description}
          placeholder={t('opcional.descricaoPlaceholder')}
          aria-invalid={problem ? true : undefined}
          aria-describedby={`${id}-hint`}
          onChange={(event) => {
            setDescription(event.target.value);
          }}
        />
        {problem ? (
          <div className="field__error" role="alert">
            <span>{problem}</span>
          </div>
        ) : null}
        <div className="field__hint" id={`${id}-hint`}>
          {t('opcional.descricaoDica')}
        </div>
      </div>
      <div className="stack-1">
        <Switch
          label={t('opcional.padrao')}
          checked={byDefault}
          onText={t('ligado')}
          offText={t('desligado')}
          onChange={setByDefault}
        />
        <div className="field__hint">{t('opcional.padraoDica')}</div>
      </div>
      {error !== undefined && error !== null ? <ErrorPanel compact error={error} /> : null}
      <div className="btn-row">
        <Button
          type="submit"
          size="sm"
          variant="primary"
          loading={saving}
          disabled={!changed || problem !== null}
        >
          {saving ? t('opcional.salvando') : t('opcional.salvar')}
        </Button>
      </div>
    </form>
  );
}
