# Prompt: Descritor Semântico no JSON do Fork

**Destino**: repositório do fork — https://github.com/Dikluwe/cargo-modules
(MPL-2.0; rust ≥ 1.91, edition 2024)
**Tipo**: modificação do fork (terceira rodada — após export-json e id)
**Criado em**: 2026-05-28
**Decisões de origem**: laudo 0010 (D4 — imprecisão trait↔id); investigação do
elo trait↔id (raiz no fork); investigação do descritor disponível
(`lab/investiga-descritor-fork/relatorio.md`); escolha do autor dos campos.

---

## Contexto

O fork já emite, por nó, `id`, `path`, `name`, `kind` (string), `visibility`,
`crate`, e por aresta `from`/`to`/`relation` + `id_from`/`id_to`. A medição e a
investigação mostraram que o fork **descarta** informação semântica que o
rust-analyzer já tem em mãos — em particular o **trait** que um método
implementa, que é o que distingue dois métodos colidentes (`Display::fmt` vs
`Debug::fmt`) e o que faltava para nomear identidades distintas com precisão.

A investigação do descritor (`lab/investiga-descritor-fork/relatorio.md`)
mapeou os campos disponíveis, seus custos, e quais padrões de colisão cada um
resolve. O autor escolheu o conjunto abaixo.

Princípio-chave do custo (da investigação, §0): como cada nó carrega o
`hir::ModuleDef` e o `Printer` tem `db: &dyn HirDatabase` em mãos, qualquer
coisa derivável de um `ModuleDef` é alcançável **no momento da serialização**,
sem mudar o `GraphBuilder`. A maioria dos campos abaixo é "já tem" ou "consulta
fácil".

---

## Restrições

- **Aditivo e retrocompatível.** TODOS os campos existentes permanecem
  inalterados — incluindo a string `kind` (ver Grupo A item 3). Os campos
  novos são acréscimos. Quem lê o JSON antigo continua funcionando.
- **Não mudar o `GraphBuilder`.** As consultas novas acontecem no `Printer`
  do `export_json` (`src/command/export_json/printer.rs`), a partir de
  `node.hir` + `db`, conforme a investigação documentou.
- **Não mudar outros subcomandos** (`structure`, `dependencies`, etc.).
- **Sem ruído em stdout.** O JSON continua parseável direto da stdout.
- **Versão**: mudança aditiva → bump minor ou patch, não major.

---

## Grupo A — sempre emitido (descritor padrão)

Estes campos entram no JSON por padrão, sem flag. Todos são "já tem" ou
"consulta fácil" (investigação Partes 1 e 2).

### A.1 — `trait` (nome do trait)

Para nós que são métodos/itens associados de um impl-de-trait ou declarados
num trait: o **nome** do trait.

- Fonte: `node.hir.as_assoc_item(db).and_then(|ai| ai.container_or_implemented_trait(db))`
  (investigação §1.4 e §2.1). Devolve o trait que declara o item (assoc item
  em `trait X`) ou que o `impl` implementa.
- Emitir como `trait: Option<String>` no nó — `null`/ausente para itens que
  não são de impl-de-trait (ex.: impls inerentes, funções livres).
- Este é o campo que resolve a D4: dois `fmt` de `Display` e `Debug` passam a
  ter `trait` distinto, e o `lente_resolve` pode nomear com precisão.

### A.2 — `trait_ref` (referência do trait com argumentos)

A referência completa do trait, **com argumentos genéricos**.

- Fonte: `Impl::trait_ref` renderizado via `HirDisplay` (investigação §2.1/§2.2).
- Emitir como `trait_ref: Option<String>` no nó — ex.: `"From<Abs>"`,
  `"From<Em>"`, `"Add"`, `"Add<f64>"`.
- Razão de ser separado do A.1: o padrão **dominante** de colisão medido
  (`From<X>+From<Y>`, `Add<Self>+Add<f64>`) é do **mesmo trait** com argumentos
  diferentes. O nome (A.1) daria "From" e "From" — não distingue. O `trait_ref`
  distingue. Os dois campos coexistem: nome para legibilidade, ref para
  precisão.

### A.3 — Modificadores de fn/trait (booleanos) — ADITIVO

Para funções: `is_const`, `is_async`, `is_unsafe`. Para traits: `is_unsafe`.

- Fonte: já computados em `src/item/kind_display_name.rs` (`is_const(db)`,
  `is_async(db)`, `is_unsafe_to_call(db,…)`, `is_unsafe(db)`) — hoje achatados
  na string `kind` (investigação §1.1).
- **ADITIVO**: emitir os booleanos como campos novos E **manter a string
  `kind` inalterada**. A string `kind` continua sendo `"const async unsafe
  fn"` como hoje. Razão: retrocompatibilidade — o `lente_infra` lê `kind` como
  string e converte por `TryFrom<&str>`; não pode quebrar. (A eventual
  remodelagem do `Kind` no projeto-lente para usar os booleanos é decisão
  futura do lado do projeto, não do fork.)
- Emitir como booleanos no nó (ausentes ou `false` quando não aplicável —
  decisão do gerador, registrar).

### A.4 — `cfg` (atributos de configuração estruturados)

A expressão `#[cfg(...)]` do item, estruturada.

- Fonte: `analyzer::cfg_attrs` → `Vec<ItemCfgAttr>` já existe e é usado por
  outros subcomandos (investigação §1.2). Já modelado recursivamente
  (`Flag`, `KeyValue`, `All`, `Any`, `Not`).
- Emitir como `cfg` no nó (estrutura ou string serializada da expressão —
  decisão do gerador; estrutura é preferível por ser parseável).

### A.5 — `macro_kind`

Para nós que são macros: o tipo (`fn-like`, `attr`, `derive`, `macro_rules!`).

- Fonte: `Macro::kind(db) -> MacroKind` (investigação §2.9), com `is_fn_like`,
  `is_attr`, `is_derive`.
- Emitir como `macro_kind: Option<String>` no nó — `null`/ausente para não-macros.

### A.6 — `is_non_exhaustive`

Marca `#[non_exhaustive]`.

- Fonte: `attrs.is_non_exhaustive` (investigação §2.6b — flag booleana pronta).
- Emitir como booleano no nó.
- Razão: é literalmente sobre raio entre-crates — `non_exhaustive` muda o que
  quebra a jusante quando o tipo muda.

---

## Grupo B — atrás de flag (descritor rico)

Estes campos são "consulta cara" e de utilidade ainda não demonstrada por
caso de colisão medido. Ficam atrás de uma **flag**, desligados por padrão.

### A flag

- Nome sugerido: `--rich` (ou `--full-descriptor` — decisão do gerador,
  registrar). A flag é **extensível**: liga "campos caros" em geral. Hoje
  controla o Grupo B abaixo; campos caros futuros (lifetimes, source span)
  entrariam sob a mesma flag.
- Sem a flag: o JSON tem só o Grupo A (+ campos pré-existentes). Com a flag:
  acrescenta o Grupo B.
- Documentar a flag no README e no help do subcomando.

### B.1 — `signature`

Tipos de parâmetros e retorno de funções, renderizados.

- Fonte: `params_without_self` + `ret_type` via `HirDisplay` (investigação §9).
- Emitir como `signature: Option<String>` no nó, só sob a flag.
- Nota de cautela (registrar no README ou comentário): a assinatura é
  **redundante** com as arestas `uses` (que já codificam os tipos
  estruturalmente) e sua renderização pode **variar entre versões do
  rust-analyzer**, o que afeta comparação entre versões. Por isso fica sob
  flag, não no padrão.

### B.2 — `generics` + `bounds`

Parâmetros genéricos do item e seus bounds.

- Fonte: `GenericDef::params`, `trait_bounds` (investigação §10).
- Emitir como estrutura sob a flag.

---

## Testes

Adicionar à suíte do fork:

- **Trait emitido**: fixture com `impl Display for X` e `#[derive(Debug)]`
  (ou impl Debug manual). Verificar que os dois `fmt` têm `trait` distinto
  (`"Display"` e `"Debug"`).
- **trait_ref com args**: fixture com `impl From<A> for T` e `impl From<B>
  for T`. Verificar que os dois `from` têm `trait_ref` distinto (`"From<A>"`,
  `"From<B>"`).
- **Aditividade do kind**: verificar que a string `kind` continua presente e
  inalterada, e que os booleanos foram adicionados ao lado.
- **Impl inerente**: verificar que método de `impl X { }` (sem trait) tem
  `trait: null`.
- **Flag desligada**: verificar que sem `--rich`, o JSON não tem `signature`
  nem `generics`.
- **Flag ligada**: verificar que com `--rich`, esses campos aparecem.
- **Retrocompatibilidade**: todos os campos pré-existentes (id, path, name,
  kind-string, visibility, from, to, relation, id_from, id_to) inalterados.

---

## README

Atualizar a seção do `export-json`:

- Novos campos do descritor padrão (Grupo A) com explicação de cada um.
- A flag `--rich` e o que ela acrescenta (Grupo B).
- Garantia de retrocompatibilidade mantida.
- Nota sobre `trait` vs `trait_ref` (nome vs referência com args) e quando
  usar cada um.
- Nota sobre derive: não é campo de atributo; aparece como impl expandido com
  `trait` preenchido, sob `--sysroot` (investigação §2.6c).

---

## Critérios de Verificação

```
Dado um tipo com impl Display e derive/impl Debug
Quando export-json (padrão, sem flag)
Então os dois nós fmt têm campo "trait" distinto ("Display", "Debug")

Dado um tipo com impl From<A> e impl From<B>
Quando export-json
Então os dois nós from têm "trait_ref" distinto ("From<A>", "From<B>")

Dado uma função const async
Quando export-json
Então o nó tem is_const=true, is_async=true E a string kind inalterada

Dado um método de impl inerente (sem trait)
Quando export-json
Então o nó tem trait=null (ou ausente)

Dado export-json SEM --rich
Quando inspecionar o JSON
Então não há campos signature nem generics

Dado export-json COM --rich
Quando inspecionar o JSON
Então signature e generics aparecem

Dado qualquer JSON gerado
Quando comparar campos pré-existentes com a versão anterior do fork
Então todos inalterados (retrocompatibilidade)
```

---

## Resultado esperado

- Modificação no `printer.rs` do `export_json` emitindo o Grupo A sempre e o
  Grupo B sob `--rich`.
- A flag `--rich` adicionada ao subcomando, extensível.
- Testes novos cobrindo os campos e a retrocompatibilidade.
- README atualizado.
- Commit descritivo citando a investigação do descritor como motivação.
- Não fazer push automaticamente (deixar para o autor).

Depois desta mudança, a cascata a jusante no projeto-lente (prompts futuros,
um de cada vez): lente_core ganha os campos novos no tipo; lente_infra os
consome; lente_investiga passa a ter o trait por nó (resolvendo a D4 do laudo
0010 na raiz); lente_resolve nomeia por trait com precisão. E, separadamente,
a eventual remodelagem do enum Kind para usar os booleanos.
