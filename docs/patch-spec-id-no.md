# Patch para `00_nucleo/forma-organizada.md`

**Propósito**: registrar o campo `id` nos nós e os campos `id_from`/`id_to`
nas arestas, depois que o fork emitir essa identidade.

**Quando aplicar**: depois que o fork tiver a modificação implementada e
medida funcionando. Não aplicar antes — manteria a spec adiante da fonte real.

**Como aplicar**: este é um patch descritivo, não diff mecânico. Você lê, edita
a spec do seu repositório (que tem os Limites 4 e 5 que eu não tenho aqui),
preservando o que ela tem hoje, e adicionando as três coisas abaixo.

---

## Mudança 1 — Esquema dos nós

Onde a spec descreve os campos de cada nó, **adicionar** o campo `id`. A
descrição sugerida:

> **`id`** (inteiro, não-negativo): identificador único do nó **dentro de um
> JSON**. Permite distinguir nós com mesmo `path` (caso comum em código Rust
> idiomático, ex.: `impl Display + derive Debug` no mesmo tipo). A
> identidade não é estável entre invocações do fork — só dentro do JSON
> emitido por uma invocação.

Posição no nó: logo no início, antes de `path`.

---

## Mudança 2 — Esquema das arestas

Onde a spec descreve os campos de cada aresta, **adicionar** os campos
`id_from` e `id_to`. A descrição sugerida:

> **`id_from`** (inteiro): o `id` do nó de origem da aresta. Sempre
> referencia o `id` de algum nó dentro do mesmo JSON.
>
> **`id_to`** (inteiro): o `id` do nó de destino da aresta. Sempre referencia
> o `id` de algum nó dentro do mesmo JSON.

Os campos `from` e `to` (path como string) permanecem **inalterados**.
Continuam sendo o caminho qualificado do nó. A diferença: `from`/`to` podem
ser ambíguos quando há colisão de path; `id_from`/`id_to` nunca são.

---

## Mudança 3 — Invariante adicional

Onde a spec lista os invariantes (atualmente: identidade por path única,
integridade referencial, valores fechados respeitados), **adicionar**:

> **Identidade por `id` (nova)**: cada `id` em `nodes` é único dentro do
> JSON. Toda aresta tem `id_from` e `id_to` que referenciam o `id` de algum
> nó em `nodes`.

E **revisar** o invariante 1 (path único), porque ele agora está em tensão
com a realidade que o fork emite. Sugestão de nova redação:

> **Identidade por `path` (revisado)**: `path` é o caminho qualificado do
> nó. **Pode haver paths repetidos em `nodes`** (ex.: dois `fmt` em impls
> diferentes de um mesmo tipo). A unicidade de identidade é garantida por
> `id`, não por `path`. O `path` mantém-se como identificador legível, mas
> não é mais o identificador formal.

---

## Mudança 4 — Limite afetado

O Limite 4 (granularidade do `uses` via import — colapso para o módulo) e o
Limite 5 (reexports colapsados em `uses`) **não mudam**. O `id` distingue
nós com mesmo path, mas não recupera a granularidade perdida quando o uso é
inferido a partir de `use` em vez de referência direta.

A Nota de Evolução já existente sobre subtipos de `uses` **ganha** uma nota
adicional: a identidade-por-nó resolve a família de ambiguidades que a
medição mediu (colisões de path); subtipos de `uses` continuam como caminho
futuro independente, para os Limites 4 e 5 que não dependem de identidade.

---

## Mudança 5 — Nota sobre a evolução

No fim da spec, antes dos Critérios de Verificação, **acrescentar** uma nota:

> **Histórico da forma**: a versão inicial da forma organizada não
> distinguia nós com mesmo path; o invariante de identidade por path era
> tido como verdade. A medição contra crates Rust reais (relatório em
> `lab/medicao-colisoes/relatorio.md`) revelou 384 colisões em 17 crates do
> typst, mostrando que o invariante anterior era idealização. A
> identidade-por-`id` foi adicionada ao fork e a esta spec como resposta
> empírica a essa descoberta. Esse padrão — spec ajustada por medição em
> Arena — é o que torna a forma organizada um contrato fiel à fonte, e não
> uma idealização separada dela.

---

## O que NÃO mudar na spec

- O esquema geral de `nodes` e `edges` como duas listas de cima do JSON.
- A lista fechada de valores para `kind`, `visibility`, `relation`.
- Os Limites 1, 2, 3 (sysroot, fronteira stdlib, raio comportamental).
- Os Limites 4 e 5 (granularidade de `uses` via import, reexports) — que
  estão no seu repositório.
- A Nota de Evolução sobre subtipos de `uses` que o Claude Code escreveu.
- Os Critérios de Verificação existentes.

Mudanças mecânicas:
- Quem lia `path` para identificar nó precisa ler `id`.
- Quem comparava arestas por `from`/`to` precisa comparar por `id_from`/`id_to`
  quando há colisão; pode continuar usando `from`/`to` quando não há.
