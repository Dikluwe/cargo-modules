# Prompt: Subtipos de `uses` no JSON do Fork (referência vs import)

**Destino**: repositório do fork — https://github.com/Dikluwe/cargo-modules
(MPL-2.0; rust ≥ 1.91, edition 2024)
**Tipo**: modificação do fork (quarta rodada — após export-json, identidade-por-nó,
descritor semântico)
**Criado em**: 2026-06-03
**Decisões de origem**: laudo 0031 (egui: 1 SCC de **85 módulos**, ≈76% do
crate); laudo 0032 (medição — a ponte-raiz/reexport **não** é a causa: remover a
raiz reduz só 85→84; hipótese rejeitada); **leitura do `src/graph/builder.rs` do
fork** (as arestas `Uses` nascem de DUAS fontes distintas no código); Limites 4
e 5 da spec da lente; a Nota de Evolução da spec (subtipos de `uses`); decisão do
autor de encarar o fork.

---

## Contexto

O fork emite, por aresta, `from`/`to`/`id_from`/`id_to` + `relation`
(`"owns"`/`"uses"`). O enum interno (`src/graph.rs`) é só `Relationship { Uses,
Owns }`. Logo, uma aresta `uses` **funde** duas coisas semanticamente
diferentes — e a leitura do construtor confirma que ambas existem:

- **Referência** — `walk_and_push_type` (`src/graph/builder.rs:526`) anda pelos
  tipos de parâmetros, campos e retornos de cada item e liga ao tipo
  referenciado (`add_edge(..., Edge::Uses)` via `add_dependencies`, linha 561).
  É o item mencionando um tipo numa assinatura/campo. Dependência genuína "X
  usa T".
- **Import** — `process_module` (`builder.rs:209-222`) itera
  `module_hir.scope(...)` e, para cada coisa no escopo que **não** é filha do
  módulo, emite uma aresta `Uses` **do módulo**. Isso é uma declaração `use`
  atribuída ao módulo. (O comentário da linha 554 diz: *"Adding outgoing 'use'
  edges"*.)

A aresta de import é **exatamente** o Limite 4 da spec (`uses` agrega imports no
nível do módulo). A medição do laudo 0032 descartou o reexport/raiz (Limite 5)
como causa do SCC de 84; o suspeito que sobra são as arestas de **import**, que
incham o grafo de módulos — um módulo que faz `use crate::context::Context;` no
topo passa a "depender" de tudo que entra no seu escopo, mesmo que só uma função
interna use.

A correção que a Nota de Evolução da spec previu é emitir **subtipos de `uses`**.
Este prompt faz a separação crítica: **referência vs import**. (Reexport é
extensão opcional — §4.)

O que isto destrava: recomputar os ciclos do egui contando **só** `reference`,
para finalmente responder se o SCC de 84 é acoplamento de tipo genuíno ou está
inflado por import — a pergunta que as últimas rodadas deixaram aberta e que só o
fork pode responder. De quebra, afina o raio de impacto da lente (o montante hoje
inclui as arestas grosseiras de import no nível do módulo).

---

## Por que esta rodada mexe no construtor (diferente do descritor)

A terceira rodada (descritor semântico) foi **só no printer**: tudo era
derivável de `node.hir` no momento da serialização, então o `GraphBuilder` ficou
intocado. **Esta rodada é mais pesada e precisa mexer no `GraphBuilder` e no
enum `Relationship`** — porque o tipo de uma aresta `uses` **não** é uma
propriedade de um nó derivável do seu `hir`; é uma propriedade de **como a
aresta foi criada**, conhecida só no ponto de criação, dentro do construtor.

A boa notícia: as duas fontes (`walk_and_push_type` e o laço de escopo de
`process_module`) **já são caminhos de código separados**. O tipo é conhecido lá
e é **jogado fora** quando ambos chamam `add_edge(Edge::Uses)`. A mudança é
carregar esse rótulo já conhecido, da criação até o printer. Localizada, mas no
nível do construtor — não só do printer.

Confirmação de que a superfície é pequena: **todas** as arestas `Uses` passam por
`add_dependencies` (único lugar que chama `add_edge(..., Edge::Uses)`, linha
561). As arestas `Owns` são criadas direto (`add_edge(..., Edge::Owns)`, linhas
96 e 205) e **não** são tocadas. Então rotular as arestas `uses` é, no
construtor, mexer em `add_dependencies` e nos seus dois chamadores.

---

## Restrições

- **Aditivo e retrocompatível no JSON.** O campo `relation` continua
  `"owns"`/`"uses"`. O subtipo é um campo **novo** nas arestas `uses`. Quem lê o
  schema antigo continua funcionando.
- **Mexe em**: o modelo (`src/graph.rs`), o construtor (`src/graph/builder.rs`)
  e o printer (`src/command/export_json/printer.rs`). **Não muda o
  comportamento** de outros subcomandos (`structure`, `dependencies`,
  `orphans`) — só o que a mudança do enum **obrigar** a compilar (ver §1) **e
  uma normalização de dedup no `dependencies` para preservar o comportamento**
  (ver §5).
- **Sem ruído em stdout.** O JSON continua parseável direto.
- **Versão**: mudança aditiva → bump minor ou patch, não major.
- **Não adiciona posições no fonte** (mudança de fork separada, para a trilha
  local).
- **Não muda quais arestas existem** — só as rotula. (Imports de prelúdio/glob,
  se já produzem arestas hoje, continuam produzindo; só passam a ser
  identificáveis como `import`.)

---

## A mudança

### 1. Modelo (`src/graph.rs`)

Fazer `Relationship` carregar o subtipo de `uses`. Sugestão (**payload**, não
variantes planas):

```rust
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Relationship {
    Owns,
    Uses(UsesKind),
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum UsesKind {
    Reference,
    Import,
    // Reexport,  // opcional — ver §4
}
```

- `Relationship` continua `Copy/Hash/Eq` (é chave de deduplicação no construtor).
- `display_name()` devolve `"uses"` para **todos** os `Uses(_)` (retrocompat: o
  `relation` no JSON e os outros subcomandos não mudam).
- Acrescentar um acessor para a string do subtipo (`"reference"`/`"import"`) que
  o printer usa.
- **Por que payload e não variantes planas** (`UsesReference`/`UsesImport`): os
  outros subcomandos que casam em `Relationship::Uses` passam a casar
  `Relationship::Uses(_)` — uma alteração de um token, comportamento idêntico.
  Variantes planas obrigariam cada `match` a enumerar todos os subtipos (mais
  ruído). O payload minimiza o impacto fora do `export_json`.

### 2. Construtor (`src/graph/builder.rs`)

A chave de deduplicação `(NodeIndex, Relationship, NodeIndex)` (linha 33) passa a
incluir o subtipo (via o enum). Consequência **correta**: uma aresta de
referência e uma de import entre o **mesmo par** viram arestas **distintas** —
são relações diferentes.

`add_dependencies` (linha 550) ganha um parâmetro `kind: UsesKind` e cria
`Edge::Uses(kind)`.

Os dois chamadores passam o subtipo que o seu caminho comprovadamente produz:

- **`process_moduledef`** (chamada na linha 176): o `module_def_hir` ainda está
  em escopo; determinar o subtipo pela variante —

  ```rust
  let kind = if matches!(module_def_hir, hir::ModuleDef::Module(_)) {
      UsesKind::Import
  } else {
      UsesKind::Reference
  };
  self.add_dependencies(*node_idx, dependencies.clone(), kind);
  ```

  Justificativa (do código): `process_module` coleta **só** dependências de
  escopo/import (o laço 209-222); **nunca** chama `walk_and_push_type`. Os
  processadores de item (`process_function`/`process_struct`/`process_enum`/
  `process_const`/`process_static`/`process_type_alias`/`process_builtin_type`/
  `process_variant`) coletam **só** dependências de `walk_and_push_type`
  (referências). Logo a variante determina o subtipo exatamente.

- **`process_impl`** (chamada na linha 129): itens de impl são funções/consts/
  type-aliases → referências → `UsesKind::Reference`.

(Alternativa: carregar o subtipo explicitamente pelo `dependencies_callback`/pela
coleção, em vez de inferir pela variante. Resultado equivalente, mais explícito,
um pouco mais de código. A inferência pela variante é correta pela estrutura do
construtor e é a mínima.)

### 3. Printer (`src/command/export_json/printer.rs`)

`JsonEdge` ganha um campo aditivo:

```rust
/// Subtipo de uma aresta `uses`: "reference" (uso direto em assinatura/campo)
/// ou "import" (declaração `use` atribuída ao módulo). Ausente para `owns`.
#[serde(skip_serializing_if = "Option::is_none")]
uses_kind: Option<&'static str>,
```

- `relation` permanece como está (`"owns"`/`"uses"`).
- Preencher `uses_kind` a partir do subtipo da aresta (`None` para `Owns`).
- Atualizar o doc-comment do schema no topo do arquivo.
- **Acrescentar `uses_kind` à chave de ordenação das arestas** (depois de
  `relation`): duas arestas `A→B` agora podem diferir só no `uses_kind` (ambas
  com `relation = "uses"`); sem isso a ordenação fica não-determinística entre
  elas.

### 4. (Opcional) Reexport — Limite 5

O laço de escopo de `process_module` produz imports (`use`) **e** reexports
(`pub use`) juntos — ambos aparecem em `module_hir.scope()` como não-filhos. Para
separar `import` de `reexport`, a **visibilidade** da entrada de escopo (pública
⇒ reexport) precisa ser obtível pela API de escopo. **Se** for fácil (uma
consulta de visibilidade na entrada), marcar as públicas como `Reexport`; senão,
deixar import e reexport fundidos sob `import`.

- É **opcional** porque o laudo 0032 mostrou que reexport **não** é o motor do
  ciclo — para o objetivo de ciclos, fundir import+reexport sob `import` é
  aceitável.
- **Não** gastar esforço grande aqui. O entregável central é referência vs
  import.

### 5. Normalização de dedup no `dependencies` (`src/command/dependencies/filter.rs`)

Descoberto na implementação (não previsto na lista de arquivos original): a
inclusão do subtipo na chave de dedup do **construtor** (§2) propaga para o
**filtro do `dependencies`**, que reconcilia arestas redundantes por
`(source, target, weight)` (linha ~207). Quando o filtro colapsa um item-filho
no módulo-pai (ex.: `--no-types`), duas arestas que antes fundiam — a referência
de um campo e o import do módulo, agora colapsadas no mesmo par — passam a ter
pesos distintos (`Uses(Reference)` vs `Uses(Import)`) e **deixam de fundir**,
emitindo duas linhas `uses` idênticas no dot (o subtipo é invisível ali).

Correção mínima e localizada: nesse dedup, chavear por
`weight.display_name()` (`"owns"`/`"uses"`) em vez do peso cru — colapsando o
subtipo **só no filtro do `dependencies`**. O `export_json` lê o grafo **direto**
(não passa por esse filtro), então continua vendo as duas arestas distintas — o
objetivo. Sem isto, o `dependencies` regrediria visualmente, violando a
restrição "outros subcomandos se comportam identicamente".

Caso de teste que pega isto: `tests/projects/github_issue_102` (módulo com `use`
**e** struct cujo campo referencia o mesmo tipo importado).

---

## Verificação

- Os testes existentes do fork passam; atualizar qualquer snapshot/expectativa
  do export-json para incluir o campo `uses_kind` (aditivo).
- **Smoke test** num crate pequeno onde um módulo tenha **as duas** coisas — uma
  declaração `use` e uma referência de tipo (o exemplo do Limite 4 da spec: um
  módulo `runner` com `use crate::parser::{tokenize};` **e** uma função cuja
  assinatura referencia um tipo):
  - a aresta módulo→item vinda do `use` sai com `uses_kind: "import"`;
  - a aresta item→tipo vinda da assinatura sai com `uses_kind: "reference"`.
- **Determinismo**: duas extrações do mesmo crate produzem JSON idêntico (a
  ordenação inclui `uses_kind`).
- `relation` inalterado (`"owns"`/`"uses"`) — consumidores antigos intactos.
- Outros subcomandos (`structure`/`dependencies`/`orphans`) compilam e se
  comportam **identicamente** (a mudança do enum é transparente para eles —
  `"uses"` continua `"uses"`).

---

## O que este prompt NÃO faz

- **Não toca a lente.** Consumir o `uses_kind` (recomputar ciclos só com
  `reference`; afinar o raio) é prompt próprio **no repositório da lente**,
  depois.
- **Não adiciona posições no fonte** (mudança de fork separada, para a trilha
  local — `module_file` é o precedente; é trabalho de printer, mais barato que
  este).
- **Não muda quais arestas existem** — só as rotula.
- **Não cria grafo de chamadas comportamental**: o construtor continua andando
  por **tipos** (`walk_and_push_type`) e **escopo** (`process_module`), nunca por
  chamadas. O Limite 3 (estrutural, não comportamental) segue.

---

## O ganho

Com o `uses_kind` emitido, a lente pode recomputar os ciclos do egui contando
**só** `reference` e ver se o SCC de 84 encolhe — a pergunta que as últimas
rodadas deixaram aberta e que só o fork podia responder. Se encolher muito, o
acoplamento "real" do egui é o resíduo, e os imports inflavam o resto. Se
encolher pouco, o acoplamento de tipo é genuíno. Em qualquer caso, a vista de
ciclos passa a ser **confiável** — e o raio local fica mais fino de quebra, pela
mesma separação.

---

## Histórico de Revisões

| Data | Motivo | Arquivos |
|------|--------|----------|
| 2026-06-03 | Subtipos de `uses` (`reference`/`import`) no export-json. `Relationship` passa a carregar `UsesKind`; o construtor rotula cada aresta conforme a fonte (`walk_and_push_type` → `reference`; laço de escopo de `process_module` → `import`), via `add_dependencies(kind)` e inferência pela variante nos dois chamadores; o printer emite `uses_kind` aditivo e o inclui na ordenação. Mexe no construtor e no enum (diferente do descritor, que foi só printer) porque o tipo da aresta só é conhecido na criação. Reexport (Limite 5) opcional. Não toca outros subcomandos no comportamento, nem a lente, nem posições, nem o grafo de chamadas. | `src/graph.rs`, `src/graph/builder.rs`, `src/command/export_json/printer.rs`, `docs/prompt-fork-subtipos-uses.md` |
| 2026-06-03 | Implementação. Como previsto, mexeu no enum (`src/graph.rs`), construtor (`src/graph/builder.rs`) e printer (`src/command/export_json/printer.rs`); o site de `match` em `dependencies/printer.rs` virou `Edge::Uses(_)`. **Adição não prevista**: §5 — a chave de dedup do `dependencies` (`filter.rs`) passou a normalizar pelo `display_name()` para preservar o comportamento (sem isso, o colapso de filhos no pai emitia `uses` duplicada). Teste novo `uses_edges_carry_reference_vs_import_subtype` (fixture `github_issue_102`). Suíte completa verde; determinismo confirmado. | `src/graph.rs`, `src/graph/builder.rs`, `src/command/export_json/printer.rs`, `src/command/dependencies/printer.rs`, `src/command/dependencies/filter.rs`, `tests/export_json.rs`, `docs/prompt-fork-subtipos-uses.md` |
