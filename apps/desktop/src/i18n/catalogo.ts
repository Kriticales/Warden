/**
 * Namespaces do catálogo (ARCHITECTURE §18). Cada arquivo de `pt-BR/` se registra sozinho:
 * `index.ts` carrega a pasta inteira (`import.meta.glob`) e o arquivo acrescenta o seu
 * namespace a esta interface com um `declare module` (veja `pt-BR/comum.ts`). Assim criar um
 * namespace não toca em nenhum arquivo compartilhado.
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type -- preenchida por augmentation
export interface Catalogo {}
