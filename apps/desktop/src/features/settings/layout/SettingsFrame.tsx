/**
 * A moldura de uma página de Configurações (cabeçalho com o título, a frase do que ela faz e as
 * ferramentas; corpo com rolagem própria) e a linha de configuração (rótulo e descrição à
 * esquerda, o controle à direita), como no esboço aprovado.
 */
import type { LucideIcon } from 'lucide-react';
import { createContext, useContext, useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PageTitle } from '../../../app/layout/PageHead';
import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import type { SettingsPageId } from '../pages';

export function SettingsFrame({
  page,
  icon,
  tools,
  children,
}: {
  page: SettingsPageId;
  /** Ícone antes do título (o ✦ do Sensor Sculk). */
  icon?: LucideIcon;
  tools?: ReactNode;
  children: ReactNode;
}) {
  const { t } = useTranslation('configuracoes');
  return (
    <section className="spage" aria-labelledby={`pagina-${page}`}>
      <div className="spage__head">
        <PageTitle className="spage__title">
          {icon ? <Icon icon={icon} className="icon--ai" /> : null}
          <span id={`pagina-${page}`}>{t(`menu.paginas.${page}`)}</span>
        </PageTitle>
        <p className="spage__sub">{t(`menu.subtitulos.${page}`)}</p>
        {tools}
      </div>
      <div className="spage__body">{children}</div>
    </section>
  );
}

/** Os ids do título e da descrição da linha, para o controle dela se rotular. */
const RowIds = createContext<{ titleId: string; descId: string | undefined } | null>(null);

/** `aria-labelledby` e `aria-describedby` de um controle dentro de `SettingRow`. */
export function useRowLabel(): { 'aria-labelledby'?: string; 'aria-describedby'?: string } {
  const ids = useContext(RowIds);
  if (!ids) return {};
  return {
    'aria-labelledby': ids.titleId,
    ...(ids.descId ? { 'aria-describedby': ids.descId } : {}),
  };
}

/** Interruptor de uma linha: o título da linha é o rótulo; o estado fica escrito ao lado. */
export function RowSwitch({
  checked,
  disabled = false,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  const { t } = useTranslation('configuracoes');
  const label = useRowLabel();
  return (
    <span className={cn('switch', disabled && 'switch--disabled')}>
      <button
        type="button"
        role="switch"
        className="switch__track"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => {
          onChange(!checked);
        }}
        {...label}
      />
      <span className="switch__state" aria-hidden="true">
        {checked ? t('ligado') : t('desligado')}
      </span>
    </span>
  );
}

/** Lista de opções de uma linha (`<select>` rotulado pelo título da linha). */
export function RowSelect({
  value,
  options,
  disabled = false,
  onChange,
}: {
  value: string;
  options: readonly { value: string; label: string }[];
  disabled?: boolean;
  onChange: (value: string) => void;
}) {
  const label = useRowLabel();
  return (
    <select
      className="select"
      value={value}
      disabled={disabled}
      onChange={(event) => {
        onChange(event.target.value);
      }}
      {...label}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function SettingRow({
  title,
  desc,
  children,
  wide = false,
  stack = false,
}: {
  title: ReactNode;
  desc?: ReactNode;
  children: ReactNode;
  /** O controle usa a largura toda da coluna (tabelas, barras). */
  wide?: boolean;
  /** Rótulo em cima, controle embaixo (a tabela de Java). */
  stack?: boolean;
}) {
  const titleId = useId();
  const descId = useId();
  return (
    <div
      className={cn('srow', wide && 'srow--wide', stack && 'srow--stack')}
      role="group"
      aria-labelledby={titleId}
    >
      <div className="srow__label">
        <span className="srow__title" id={titleId}>
          {title}
        </span>
        {desc ? (
          <span className="srow__desc" id={descId}>
            {desc}
          </span>
        ) : null}
      </div>
      <div className="srow__control">
        <RowIds.Provider value={{ titleId, descId: desc ? descId : undefined }}>
          {children}
        </RowIds.Provider>
      </div>
    </div>
  );
}
