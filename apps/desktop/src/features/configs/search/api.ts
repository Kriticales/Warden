/**
 * Dados da busca em todas as configs (C-05; ARCHITECTURE §10.1): `config_search` e o evento
 * `config-index-changed`.
 *
 * As chaves ficam sob a área `configs` do pack: o `pack-changed` que o Rust emite quando um
 * arquivo muda por fora invalida a busca junto com a árvore e o arquivo aberto. Enquanto o
 * índice faz a primeira leitura, a busca é repetida para mostrar o que já foi lido; quando ele
 * fica pronto (ou muda), `config-index-changed` manda buscar de novo.
 */
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import {
  commands,
  events,
  type ConfigOrigin,
  type ConfigSearchResponse,
  type PackId,
} from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { unwrap } from '../../../lib/ipc/result';
import { configsKeys } from '../api';

/** Espera depois da última tecla antes de buscar. */
export const TYPING_DELAY_MS = 200;

/** Intervalo entre as buscas enquanto o índice faz a primeira leitura (ou depois de uma busca
 * cancelada por outra). */
const READING_POLL_MS = 500;

/** A resposta e o texto buscado. */
export interface SearchAnswer extends ConfigSearchResponse {
  query: string;
}

/**
 * O texto depois de [`TYPING_DELAY_MS`] sem mudar; `null` até a primeira espera terminar. Os
 * resultados aparecem na primeira tecla, então a espera vale desde ela (o `useDebouncedValue`
 * do Histórico começa com o valor inicial e buscaria a primeira letra sozinha).
 */
export function useSettledText(value: string): string | null {
  const [settled, setSettled] = useState<string | null>(null);
  useEffect(() => {
    const timer = setTimeout(() => {
      setSettled(value);
    }, TYPING_DELAY_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [value]);
  return settled;
}

export const searchKeys = {
  /** Toda busca de uma origem. */
  origin: (packId: PackId, origin: ConfigOrigin) =>
    [...configsKeys.all(packId), origin, 'search'] as const,
  query: (
    packId: PackId,
    origin: ConfigOrigin,
    query: string,
    modId: string | null,
    onlyChanged: boolean,
  ) => [...searchKeys.origin(packId, origin), query, modId, onlyChanged] as const,
};

/**
 * Busca `query` (já com a espera da última tecla) na origem. Busca vazia só prepara o índice
 * e devolve a situação dele (a tela chama ao abrir), a não ser com "Só o que mudou do padrão"
 * (C-06), que então lista todas as chaves alteradas. Enquanto a próxima resposta não chega, a
 * anterior continua na tela.
 */
export function useConfigSearch(
  packId: PackId,
  origin: ConfigOrigin,
  query: string,
  modId: string | null,
  enabled = true,
  onlyChanged = false,
) {
  return useQuery({
    queryKey: searchKeys.query(packId, origin, query, modId, onlyChanged),
    // A busca vai junto da resposta: enquanto a próxima não chega, a tela diz de qual é.
    queryFn: async (): Promise<SearchAnswer> => ({
      ...unwrap(await commands.configSearch(packId, origin, query, modId, onlyChanged)),
      query,
    }),
    enabled,
    staleTime: 0,
    placeholderData: keepPreviousData,
    refetchInterval: (current) =>
      current.state.data && (!current.state.data.status.ready || current.state.data.cancelled)
        ? READING_POLL_MS
        : false,
  });
}

/** Busca de novo quando o índice da origem termina a primeira leitura ou muda. */
export function useConfigIndexEvents(packId: PackId) {
  const queryClient = useQueryClient();
  useTauriEvent(events.configIndexChanged, (change) => {
    if (change.packId === packId) {
      void queryClient.invalidateQueries({ queryKey: searchKeys.origin(packId, change.origin) });
    }
  });
}
