/**
 * O campo de pergunta do Sensor Sculk, fixo embaixo da conversa (protótipo
 * `design/bug-01/sensor-sculk.html`): a caixa cresce com o texto (até 200 px), **Enter** envia e
 * **Shift+Enter** quebra a linha; na barra, **Procurar rápido | Procurar a fundo** à esquerda
 * (só no Claude e na OpenAI; no Gemini não aparece, ADR-0064) e **Enviar** à direita. Enviar fica
 * indisponível, com a dica do porquê, sem texto, enquanto o Sculk lê ou quando a pergunta
 * anterior ainda espera resposta.
 */
import { Send } from 'lucide-react';
import { useEffect, useId, useRef, useState, type SyntheticEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Tooltip } from '../../../components/ui/tooltip';
import type { AiProvider, SearchMode } from '../../../lib/ipc/bindings';
import { toAppError } from '../../../lib/ipc/errors';
import { aiErrorMessage } from '../../settings/ai/providers';
import { ModeSelect } from './ModeSelect';

/** O máximo de uma pergunta (o mesmo do backend). */
const MAX_QUESTION = 8000;

export function Composer({
  provider,
  reading,
  waitingAnswer,
  initialMode,
  sending,
  error,
  onSend,
  placeholder,
}: {
  provider: AiProvider;
  /** O Sculk está lendo nesta conversa. */
  reading: boolean;
  /** A última pergunta não tem resposta (parou com erro ou foi interrompida). */
  waitingAnswer: boolean;
  /** O modo proposto (o da última pergunta). */
  initialMode: SearchMode;
  sending: boolean;
  error: unknown;
  /** Manda a pergunta; resolve quando foi aceita (o campo esvazia). */
  onSend: (text: string, mode: SearchMode | null) => Promise<unknown>;
  placeholder?: string | undefined;
}) {
  const { t } = useTranslation('ia');
  const id = useId();
  const area = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState('');
  const [mode, setMode] = useState<SearchMode>(initialMode);
  const modes = provider !== 'gemini';
  const blocked = reading || waitingAnswer || sending;
  const ready = text.trim() !== '' && !blocked;
  const why = reading
    ? t('composer.dicaLendo')
    : waitingAnswer
      ? t('composer.depoisDoErro')
      : text.trim() === ''
        ? t('composer.dicaVazio')
        : null;

  useEffect(() => {
    const element = area.current;
    if (!element) return;
    element.style.height = 'auto';
    element.style.height = `${String(Math.min(element.scrollHeight, 200))}px`;
  }, [text]);

  const submit = (event?: SyntheticEvent) => {
    event?.preventDefault();
    if (!ready) return;
    onSend(text, modes ? mode : null)
      .then(() => {
        setText('');
      })
      .catch(() => {
        // O erro aparece logo abaixo do campo.
      });
  };

  const send = (
    <Button
      type="submit"
      variant="primary"
      icon={Send}
      className="cbar__send"
      loading={sending}
      aria-disabled={!ready}
      onClick={(event) => {
        if (!ready) event.preventDefault();
      }}
    >
      {t('composer.enviar')}
    </Button>
  );

  return (
    <form className="composer" onSubmit={submit}>
      <div className="composer__inner">
        {error ? (
          <Alert kind="danger" compact>
            {aiErrorMessage(toAppError(error), provider)}
          </Alert>
        ) : null}
        <div className="cbox">
          <label className="sr-only" htmlFor={id}>
            {t('composer.rotulo')}
          </label>
          <textarea
            ref={area}
            id={id}
            rows={1}
            maxLength={MAX_QUESTION}
            value={text}
            spellCheck={false}
            placeholder={placeholder ?? t('composer.exemplo')}
            onChange={(event) => {
              setText(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
                event.preventDefault();
                submit();
              }
            }}
          />
          <div className="cbar">
            {modes ? <ModeSelect value={mode} onChange={setMode} disabled={sending} /> : null}
            {why ? <Tooltip content={why}>{send}</Tooltip> : send}
          </div>
        </div>
      </div>
    </form>
  );
}
