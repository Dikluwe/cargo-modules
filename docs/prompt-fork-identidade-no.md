# Prompt: Adicionar Identidade-Por-Nó ao JSON do Fork

**Destino**: repositório do fork — https://github.com/Dikluwe/cargo-modules
(MPL-2.0; rust ≥ 1.91, edition 2024)
**Tipo**: modificação do fork (projeto externo ao projeto-lente)
**Criado em**: 2026-05-27
**Decisões de origem**: medição de prevalência de colisões (`lab/medicao-colisoes/relatorio.md`
do projeto-lente), que revelou que a Estratégia 1 do `lente_investiga` é
inaplicável porque o JSON do fork referencia arestas por path, perdendo a
distinção quando há colisão.

---

## Contexto

O fork do `cargo-modules` em https://github.com/Dikluwe/cargo-modules adicionou
o subcomando `export-json` que serializa o grafo interno do `cargo-modules`
(estrutura do rust-analyzer) como JSON. O formato atual emite:

- Cada **nó** com campos: `path`, `name`, `kind`, `visibility`, `crate`.
- Cada **aresta** com campos: `from` (path), `to` (path), `relation`.

A medição realizada no projeto-lente, executada contra 17 crates do typst
(v0.14.2), encontrou 384 colisões — múltiplos nós com o mesmo `path`. Quando
existem n nós com o mesmo `path` e o cargo-modules emite uma aresta envolvendo
"aquele path", a aresta aponta para "o path" sem distinguir qual das n cópias
é a origem ou o destino real da relação.

Isso significa que **informação que existe no grafo interno do cargo-modules
(que nó específico é cada extremo da aresta) é perdida na serialização**. Para
o projeto-lente, isso impossibilita resolver colisões pelo padrão de
vizinhança no grafo.

A correção é estrutural: serializar a identidade interna do nó, e fazer as
arestas referenciarem essa identidade além do path.

---

## Restrições

- **Modificação aditiva, retrocompatível.** Os campos atuais (`path`, `from`,
  `to`) permanecem como estão. A mudança apenas **adiciona** campos novos:
  `id` nos nós, `id_from` e `id_to` nas arestas. Quem lê o JSON antigo só com
  `path` continua funcionando; quem precisa de identidade por nó usa os novos
  campos.
- **`id` é estável dentro de uma única invocação.** Não é exigido que o mesmo
  nó receba o mesmo `id` em invocações diferentes. Estabilidade entre
  invocações é critério mais forte e não é necessário para o caso de uso.
- **`id` é o índice interno do nó na estrutura do `cargo-modules` (provavelmente
  `petgraph`/`StableGraph`), ou um sequencial atribuído no momento da
  serialização.** A escolha entre os dois é decisão de implementação — o que
  importa é que dentro de um JSON, o `id` identifica unicamente um nó, e as
  arestas usam o `id` certo para cada extremo.
- **Sem mudar o que o `cargo-modules` extrai do código.** Esta mudança é só
  na serialização — preservar a identidade interna dos nós ao virar JSON.
  Nenhuma análise nova do rust-analyzer.
- **Sem alterar o subcomando, as flags, ou outros formatos de saída** (DOT,
  tree). A mudança é local ao caminho de serialização do `export-json`.

---

## O que fazer

### Mudança no formato JSON

**Nós** ganham um campo `id` (inteiro não-negativo):

```jsonc
// Antes
{
  "path": "lente_core::domain::raio::ErroRaio::fmt",
  "name": "fmt",
  "kind": "fn",
  "visibility": "priv",
  "crate": "lente_core"
}

// Depois
{
  "id": 42,
  "path": "lente_core::domain::raio::ErroRaio::fmt",
  "name": "fmt",
  "kind": "fn",
  "visibility": "priv",
  "crate": "lente_core"
}
```

**Arestas** ganham campos `id_from` e `id_to` (inteiros), correspondendo aos
`id` dos nós que elas conectam:

```jsonc
// Antes
{
  "from": "lente_core::domain::raio",
  "to": "lente_core::domain::raio::ErroRaio",
  "relation": "owns"
}

// Depois
{
  "from": "lente_core::domain::raio",
  "id_from": 15,
  "to": "lente_core::domain::raio::ErroRaio",
  "id_to": 28,
  "relation": "owns"
}
```

### Invariante a manter

Em qualquer JSON emitido, para toda aresta: deve existir um nó com `id ==
id_from` e um nó com `id == id_to`. O `id` referencia exatamente um nó dentro
do JSON.

### Onde tocar no código

A invocação `cargo modules export-json --sysroot --compact --package <nome>`
deve continuar funcionando exatamente como antes (mesmas flags, mesmo
comportamento), apenas com os campos adicionais.

A serialização provavelmente está em alguma função `to_json` ou módulo
`printer/json` do `cargo-modules`. O código já itera sobre os nós do grafo
interno antes de emiti-los; basta:

1. Atribuir um `id` a cada nó durante a iteração (incrementando um contador,
   ou usando o índice estável que o `petgraph` já fornece).
2. Manter um mapa `node_handle → id` durante a serialização.
3. Ao emitir cada aresta, consultar o mapa para preencher `id_from` e
   `id_to` correspondentes aos nós das pontas.

### Testes

Adicionar ao menos:

- **Teste de invariante**: para um crate-fixture simples, gerar o JSON e
  verificar que todo `id_from` e `id_to` de toda aresta corresponde a um
  `id` de algum nó.
- **Teste de unicidade**: verificar que dois nós no mesmo JSON nunca têm o
  mesmo `id`.
- **Teste do caso de colisão**: usar um fixture que tenha colisão de path
  (ex.: um enum com `derive Debug + impl Display`). Verificar que os dois
  nós colidentes têm `id` diferentes, e que as arestas envolvendo cada um
  apontam para `id_from`/`id_to` correto, distintos entre si.

### Documentação

Atualizar o README do fork mencionando:

- O subcomando `export-json` agora emite `id` nos nós e `id_from`/`id_to` nas
  arestas, além dos campos pré-existentes.
- O propósito: permitir que ferramentas a jusante distingam nós com mesmo
  path (caso comum em código Rust com `derive`, impls genéricos, etc.).
- Garantia: `id` é único dentro de um JSON; estabilidade entre invocações
  não é garantida.

---

## Critérios de Verificação

```
Dado um crate simples sem colisões de path
Quando rodar cargo modules export-json --sysroot --compact --package <nome>
Então cada nó do JSON tem campo "id" (inteiro)
E cada aresta tem campos "id_from" e "id_to" (inteiros)
E para toda aresta, existem nós com id == id_from e id == id_to

Dado um crate com colisão de path (ex.: enum com derive Debug + impl Display)
Quando rodar export-json
Então os nós colidentes têm "id" diferentes entre si
E as arestas envolvendo cada um deles têm id_from/id_to correspondentes

Dado o mesmo JSON
Quando comparar com a saída do export-json antes desta mudança
Então todos os campos antigos permanecem inalterados (retrocompatibilidade)
E apenas os campos novos "id", "id_from", "id_to" foram acrescentados
```

---

## Restrições adicionais

- **Sem mudar a versão major do fork.** Esta é mudança aditiva, não-quebrante.
  Versão semântica adequada (minor ou patch, conforme convenção do fork).
- **Sem mexer no comportamento de outros subcomandos** (`generate`,
  `dependencies`, etc., se existirem). A mudança é estritamente no caminho
  de serialização JSON.
- **Sem deixar mensagens novas em stdout/stderr.** O JSON precisa continuar
  parseável diretamente da stdout sem ruído.

---

## Resultado esperado

- Modificação no código do fork emitindo os campos novos.
- Testes novos verificando os invariantes.
- README atualizado.
- Commit (ou PR para si mesmo) descrevendo a mudança e citando o relatório
  da medição como motivação.

Depois desta mudança rodar no fork, o projeto-lente fará duas tarefas
correlatas (em prompts separados, futuros):

1. Atualizar o `lente_infra` para ler e usar `id_from`/`id_to` em vez de
   inferir de `from`/`to` quando há colisão.
2. Atualizar a Estratégia 1 do `lente_investiga` para usar a identidade-por-nó
   no critério de vizinhança (ela passará a poder decidir de verdade).
3. Refazer a medição contra o fork novo, para confirmar que a cobertura sobe
   do patamar atual (14.3%).
