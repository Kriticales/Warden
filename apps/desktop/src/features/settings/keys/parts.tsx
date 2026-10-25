/**
 * Peças comuns das chaves: a etiqueta da situação, o final `••••xxxx` e o passo a passo
 * "Como criar" com o link da página oficial.
 */
import { CircleCheck, CircleX, ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Tooltip } from '../../../components/ui/tooltip';
import { openExternal } from '../../../lib/external';
import type { KeyOverview, SecretKind } from '../../../lib/ipc/bindings';
import { isToken, type KeyState } from './model';

/** Onde criar cada chave. */
export const KEY_PAGES: Record<SecretKind, string> = {
  curseforge: 'https://console.curseforge.com/#/api-keys',
  gemini: 'https://aistudio.google.com/apikey',
  openai: 'https://platform.openai.com/api-keys',
  anthropic: 'https://platform.claude.com/settings/keys',
  github: 'https://github.com/settings/personal-access-tokens/new',
};

/** O carregador em blocos do design system (dentro de etiquetas e botões). */
export function BlockLoader() {
  return (
    <span className="loader" aria-hidden="true">
      <i />
      <i />
      <i />
      <i />
    </span>
  );
}

export function KeyStateTag({ kind, state }: { kind: SecretKind; state: KeyState }) {
  const { t } = useTranslation('configuracoes');
  switch (state) {
    case 'ok':
      return (
        <span className="tag tag--ok">
          <Icon icon={CircleCheck} />
          {t('chaves.estado.ok')}
        </span>
      );
    case 'refused':
      return (
        <span className="tag tag--danger">
          <Icon icon={CircleX} />
          {t('chaves.estado.refused')}
        </span>
      );
    case 'testing':
      return (
        <span className="tag tag--busy">
          <BlockLoader />
          {t('chaves.estado.testing')}
        </span>
      );
    case 'untested':
      return <span className="tag">{t('chaves.estado.untested')}</span>;
    case 'none':
      return (
        <span className="tag">
          {isToken(kind) ? t('chaves.estado.noneToken') : t('chaves.estado.noneChave')}
        </span>
      );
  }
}

/** `••••xxxx`, só os últimos 4 caracteres; sem chave, um traço. */
export function KeyMask({
  kindToken,
  keyInfo,
}: {
  kindToken: boolean;
  keyInfo?: KeyOverview | undefined;
}) {
  const { t } = useTranslation('configuracoes');
  if (!keyInfo?.configured) {
    return (
      <span className="keymask keymask--none" aria-hidden="true">
        —
      </span>
    );
  }
  return (
    <Tooltip content={kindToken ? t('chaves.finalDicaToken') : t('chaves.finalDica')}>
      <span className="keymask">••••{keyInfo.hint ?? ''}</span>
    </Tooltip>
  );
}

export function HowTo({ kind, title }: { kind: SecretKind; title?: string }) {
  const { t } = useTranslation('configuracoes');
  return (
    <div className="howto">
      {title ? <h3 className="howto__title">{title}</h3> : null}
      <ol>
        {t(`chaves.passos.${kind}`, { returnObjects: true }).map((step) => (
          <li key={step}>
            <span>{step}</span>
          </li>
        ))}
      </ol>
      <Button
        variant="link"
        icon={ExternalLink}
        onClick={() => {
          void openExternal(KEY_PAGES[kind]);
        }}
      >
        {t('chaves.abrirPagina')}
      </Button>
    </div>
  );
}
