# Relatório — Descritor Semântico Disponível no Fork

**Tipo**: investigação de leitura (bancada / `lab/`). Não modifica o fork nem
o projeto-lente.
**Prompt de origem**: `docs/prompt-investiga-descritor-fork.md`.
**Data**: 2026-05-28.
**Alvo lido**: fork `cargo-modules` (branch `main`, `cargo-modules 0.26.0`) e a
API do `rust-analyzer` que ele usa — crates `ra_ap_* = 0.0.328`
(`ra_ap_hir` em particular), fonte vendida em
`~/.cargo/registry/src/.../ra_ap_hir-0.0.328/`.

> **Escopo e limite.** Este relatório **mapeia** o que está disponível por
> item e a que custo. Ele **não escolhe** o descritor e **não recomenda** um
> subconjunto — essa decisão é do autor. Os agrupamentos no fim existem só
> para facilitar a leitura, não para decidir.

---

## 0. Como o fork prende a informação hoje (contexto para custo)

Tudo gira em torno de uma estrutura mínima. Cada nó do grafo é um `Item`
(`src/item.rs`) que guarda **um único campo**:

```rust
pub struct Item {
    pub hir: hir::ModuleDef,
}
```

`hir::ModuleDef` é o handle do rust-analyzer para o item. **Isso é decisivo
para o custo de qualquer campo novo**: como o nó carrega o `ModuleDef`, e o
`Printer` de JSON tem `db: &dyn HirDatabase` e `edition` em mãos
(`src/command/export_json/printer.rs`), **qualquer coisa que o rust-analyzer
saiba derivar de um `ModuleDef` é alcançável no momento da serialização**, sem
mudar o `GraphBuilder` nem guardar estado extra no nó. A pergunta de custo
quase nunca é "o dado existe?" (existe), e sim "quão cara é a consulta e quão
verboso/instável é renderizar o resultado em string".

Quatro faixas de custo, usadas no resto do relatório:

| Faixa | Significado |
|-------|-------------|
| **já tem** | O valor já é computado em algum ponto do pipeline (ou já está armazenado). Emitir = reorganizar a saída; **zero** consultas novas ao RA. |
| **consulta fácil** | Uma chamada barata e cacheada (salsa) a partir de `node.hir` + `db`, sobre dados já carregados. Sem plumbing novo. |
| **consulta cara** | Exige encanamento extra (VFS, `LineIndex`) **ou** renderizar tipos complexos / fazer várias consultas e montar string estável. |
| **exposto, mas indireto** | O RA expõe o fato, porém por um caminho diferente do ingênuo (ex.: derives não são uma lista de strings; são impls expandidos). |

O precedente do campo `id` confirma a leitura: ele era o índice do `petgraph`
já existente — "já tem" puro — e por isso foi barato. O `trait`, alvo desta
investigação, está quase tão barato quanto, como a Parte 1 mostra.

---

## 1. Parte 1 — O que o `cargo-modules` já tem na mão e não emite

Estes são os campos baratos: o fork **já computa ou já tem acesso trivial** a
eles durante a construção do grafo ou na renderização, mas o
`export-json` (`printer.rs`) só emite `id`, `path`, `name`, `kind`,
`visibility`.

### 1.1. Modificadores de `fn` e `trait` — **já computados, hoje achatados em `kind`**

`src/item/kind_display_name.rs` **já consulta**, para cada função:
`is_const(db)`, `is_async(db)`, `is_unsafe_to_call(db, …)`; e para cada trait:
`is_unsafe(db)`. Mas o resultado é **achatado numa única string** `kind`
(ex.: `"const async unsafe fn"`, `"unsafe trait"`). O dado estruturado
(três/quatro booleanos) já está em mãos no momento em que a string é montada —
emiti-lo como campos separados é **reorganização pura, zero consulta nova**.

- Disponibilidade: **já tem**.
- Nuance: hoje vêm como blob textual; quem consome o JSON tem que parsear a
  string `kind` para recuperá-los.

### 1.2. Atributos `cfg` — **já extraídos, usados por outros subcomandos, não emitidos aqui**

`analyzer::cfg_attrs` → `Vec<ItemCfgAttr>` (`src/item/attr.rs`) já modela
`cfg(...)` recursivamente (`Flag`, `KeyValue`, `All`, `Any`, `Not`) e é
consumido pelos temas de `structure`/`dependencies`. O `ItemAttrs::new`
(`src/item/attr.rs`) já agrega `cfgs` por item. O `export-json` **não** emite.

- Disponibilidade: **já tem** (código de extração pronto e testado no projeto).

### 1.3. Atributo de teste (`#[test]`) — **já extraído, não emitido**

`analyzer::test_attr` → `Option<ItemTestAttr>`, também já agregado em
`ItemAttrs.test`. Marca funções de teste. Não emitido pelo `export-json`.

- Disponibilidade: **já tem**.

### 1.4. Trait implementado/declarante — **em mãos durante o build, e barato de reconsultar**

Este é o campo que motivou a investigação (laudo 0010 / D4). Duas observações:

1. **O builder já tem o `impl` na mão e descarta o trait.** Em
   `GraphBuilder::process_crate` (`src/graph/builder.rs:76`) o fork itera
   `hir::Impl::all_in_crate(...)`, pega `impl_hir.self_ty(db)` — e **nunca
   chama `impl_hir.trait_(db)`**. Pior: em `analyzer::assoc_item_path`
   (`src/analyzer.rs:569`) o código **faz match** em
   `AssocItemContainer::Impl(impl_hir)` e usa só `impl_hir.self_ty(...).as_adt()`
   para montar o path — o trait está literalmente ali, ignorado. Por isso dois
   métodos `fmt` (de `Display` e de `Debug`) no mesmo tipo recebem o **mesmo
   `path`** e só se distinguem por `id`.
2. **Reconsultar no momento da serialização é barato.** Como o nó carrega o
   `ModuleDef`, o `Printer` pode fazer, sem tocar o builder:
   `node.hir.as_assoc_item(db).and_then(|ai| ai.container_or_implemented_trait(db))`.
   `container_or_implemented_trait` (ver Parte 2) é um único método público que
   devolve o trait — seja o trait que **declara** o item (assoc item dentro de
   `trait X`), seja o trait que o `impl` **implementa**.

- Disponibilidade: **já tem / consulta fácil** (em mãos no build; trivial de
  reconsultar na serialização).

### 1.5. Caminho do módulo-pai / contêiner

A visibilidade já navega `hir.module(db)` e `parent(db)`
(`src/item/visibility.rs`). O módulo-pai de qualquer nó é alcançável da mesma
forma. É em boa parte **redundante** com `path` (que já é qualificado), mas
está listado por completude.

- Disponibilidade: **já tem / consulta fácil**.

---

## 2. Parte 2 — O que o rust-analyzer expõe e a que custo

Tudo abaixo é **público** em `ra_ap_hir 0.0.328` e alcançável a partir de
`node.hir` + `db`. A divisão é por **custo de consulta + custo de renderização
estável**, não por disponibilidade (quase tudo está disponível).

### 2.1. Trait do `impl` / trait declarante — **consulta fácil**

API confirmada em `ra_ap_hir-0.0.328/src/lib.rs`:

- `Impl::trait_(self, db) -> Option<Trait>` (`lib.rs:4939`) — o trait que um
  `impl` implementa (`None` para impls inerentes).
- `AssocItem::container(db) -> AssocItemContainer` (`lib.rs:3902`), com
  `AssocItemContainer::{Trait, Impl}`.
- Conveniências de uma chamada (`lib.rs:3924–3949`):
  - `AssocItem::container_trait(db) -> Option<Trait>` (só quando o item é
    declarado dentro de `trait X`);
  - `AssocItem::implemented_trait(db) -> Option<Trait>` (só quando o item está
    num `impl Trait for T`);
  - **`AssocItem::container_or_implemented_trait(db) -> Option<Trait>`** — o
    caso geral, cobre os dois.
- `ModuleDef::as_assoc_item(db) -> Option<AssocItem>` (`lib.rs:3780`) é a ponte
  do que o nó carrega para essas APIs.

Nuance importante para colisões (ver Parte 3): `trait_()` devolve o **trait
(nome)**. Isso distingue `Display` de `Debug`, mas **não** distingue
`From<X>` de `From<Y>` nem `Add<Self>` de `Add<f64>` — nesses casos o nome do
trait é o mesmo (`From`, `Add`) e o que difere são os **argumentos genéricos**
do trait. Para esses, é preciso `Impl::trait_ref(self, db) -> Option<TraitRef>`
(`lib.rs:4954`), que carrega a referência completa (com args) e implementa
`HirDisplay` (`display.rs:847`) — renderizável como `From<X>` / `From<f64>`.

- Custo: o **nome** do trait é consulta fácil; a **referência com args**
  (`trait_ref` + render) é um degrau acima (renderização), mas ainda barato.

### 2.2. Assinatura de função (tipos de parâmetros e retorno) — **consulta cara (renderização)**

O builder **já anda** por esses tipos para criar arestas `uses`
(`process_function`, `src/graph/builder.rs:243–267`):
`function_hir.params_without_self(db)`, `assoc_fn_params(db)`,
`ret_type(db)` — cada um devolve `Param`/`Type`. Ou seja, os `Type` estão ao
alcance. APIs: `Function::ret_type` (`lib.rs:2546`), `params_without_self`
(`lib.rs:2651`), `self_param`/`has_self_param`, `num_params`, e
`Function::ty` (`lib.rs:2409`) para o tipo da função inteira.

O custo não é obter os tipos — é **renderizar uma string estável**. `Type`
implementa `HirDisplay` (`display.rs:530`); renderizar exige um
`DisplayTarget`/edition e produz texto que é **verboso e sensível à edição**.
A assinatura também é **parcialmente redundante** com as arestas `uses`, que já
codificam os tipos referenciados (sem ordem nem posição, é verdade).

- Disponibilidade: exposto; **consulta cara** pela renderização + estabilidade.

### 2.3. Generics e bounds — **consulta cara**

`GenericDef` (`lib.rs:4046`) com:
- `params(db) -> Vec<GenericParam>` (`lib.rs:4071`);
- `lifetime_params(db) -> Vec<LifetimeParam>` (`lib.rs:4091`);
- `type_or_const_params(db) -> Vec<TypeOrConstParam>` (`lib.rs:4103`).

Bounds de trait: `Trait::trait_bounds(db) -> Vec<Trait>` (`lib.rs:4685`) e os
bounds de `impl Trait`/`dyn Trait` que o builder já encontra em
`walk_and_push_type` (`as_impl_traits`, `as_dyn_trait`,
`as_associated_type_parent_trait`, `src/graph/builder.rs:537–547`).

Conversão `ModuleDef`→`GenericDef`: via `From<GenericDefId>`
(`from_id.rs:197`) cobrindo Function/Adt/Trait/TypeAlias/Impl/Const/Static.

- Disponibilidade: exposto; **consulta cara** (várias consultas + render por
  parâmetro/bound; texto sensível à edição).

### 2.4. Associated types (caso `HtmlElem::Type`) — **coberto pela 2.1, não é campo próprio**

Um associated type é um `hir::ModuleDef::TypeAlias` cujo `container` é um
`Impl` (ou `Trait`). Logo a **mesma** API de 2.1
(`as_assoc_item` → `container_or_implemented_trait`) já nomeia o trait que o
desambigua. A colisão `HtmlElem::Type` (vários `type Type = …` em impls de
traits diferentes no mesmo tipo) **não precisa de campo novo** além do trait.

- Disponibilidade: **consulta fácil**, idêntica a 2.1.

### 2.5. Lifetimes — **consulta cara, utilidade duvidosa para a lente**

`GenericDef::lifetime_params(db) -> Vec<LifetimeParam>` (`lib.rs:4091`), cada
`LifetimeParam` com nome. Exposto, mas no nível de granularidade do
grafo-de-módulos lifetimes raramente mudam a resposta "o que quebra se eu
mexer aqui". (Ver filtro na Parte 3.)

- Disponibilidade: exposto; **consulta cara**, utilidade baixa.

### 2.6. Atributos — três sabores, custos diferentes

`HasAttrs::attrs(db) -> AttrsWithOwner` (`attrs.rs:162`) está implementado para
praticamente todos os itens (Adt, Function, Trait, Const, Static, TypeAlias,
EnumVariant, Macro, Module, …).

a) **`cfg`** — já tratado na Parte 1.2 (**já tem**). Também
   `AttrsWithOwner::cfgs(db)` (`attrs.rs:139`).

b) **Flags booleanas prontas** (`attrs.rs:83–116`) — **consulta fácil**, são
   bits já computados: `is_deprecated`, `is_doc_hidden`, `is_unstable`,
   `is_non_exhaustive`, `is_test`, `is_macro_export`, `is_doc_notable_trait`.
   Mais `lang(db)` (lang item) e `doc_aliases(db)`.

c) **`#[derive(...)]` como lista de nomes** — **exposto, mas indireto.** Esta
   versão do `ra_ap_hir` modela atributos como um **bitset `AttrFlags`**, não
   como uma lista de atributos arbitrários iteráveis em string. Não há um
   "dê-me os nomes dos derives" direto. O caminho **honesto** para derives é o
   que o próprio fork já usa: o rust-analyzer **expande** cada derive num
   `impl` sintetizado, que aparece como nó, e cujo `Impl::trait_(db)` nomeia o
   trait derivado (`Clone`, `Debug`, …). Isso depende de `--sysroot`
   (documentado em `docs/relatorio-export-json.md` §5). Ou seja: "derive" não é
   um campo de atributo por item; é a combinação **nó-impl-expandido + campo
   trait (2.1)**.

### 2.7. Docstrings — **consulta fácil, utilidade baixa para a lente**

`HasAttrs::hir_docs(db) -> Option<&Docs>` (`attrs.rs:180`) /
`AttrsWithOwner::hir_docs` (`attrs.rs:151`). Texto da doc do item. Barato, mas
é descrição humana, não raio de impacto.

- Disponibilidade: **consulta fácil**, utilidade baixa.

### 2.8. Span de origem (arquivo + linha) — **consulta cara, plumbing**

`HasSource::source(db) -> Option<InFile<Ast>>` e
`source_with_range(db) -> Option<InFile<(TextRange, …)>>`
(`has_source.rs:21–44`) dão o nó-fonte e o **range em bytes**. Mas:
- converter `InFile`→caminho de arquivo exige a **VFS** (o `Printer` hoje
  **não** a tem; o `Command` tem — o padrão está em `analyzer::module_file`,
  `src/analyzer.rs:744`);
- converter offset de byte→linha/coluna exige um **`LineIndex`** (consulta
  adicional ao `ide_db`).

É factível, mas é o candidato com **mais encanamento** e o que **menos serve à
pergunta da lente** (é metadado de localização). O prompt pede para registrar,
sem descartar, a utilidade futura: serviria a uma feature de
**"abrir no editor"** / navegação de volta ao código — já anotada como próximo
passo opcional em `docs/relatorio-export-json.md` §11.

- Disponibilidade: exposto; **consulta cara** (VFS + LineIndex).

### 2.9. Kind de macro — **consulta fácil** (desambigua macros)

`Macro::kind(db) -> MacroKind` (`lib.rs:3508`), com `is_fn_like` (`lib.rs:3534`),
`is_attr` (`lib.rs:3578`), `is_derive` (`lib.rs:3582`). Distingue macro
declarativa (`macro_rules!`) de proc-macros (function-like / attribute /
derive).

- Disponibilidade: **consulta fácil**.

---

## 3. Parte 3 — Tabela de candidatos

Filtro da pergunta-âncora: **"este campo ajuda a responder *o que quebra se eu
mexer aqui* — o raio de impacto — ou é informação interessante mas irrelevante
a isso?"** A coluna "Serve à lente?" aplica esse filtro; a última coluna conecta
com os padrões de colisão medidos (`Display+Debug`, `From<X>+From<Y>`,
`Add<Self>+Add<f64>`, associated types `HtmlElem::Type`, macros).

| # | Campo candidato | O que é | Disponibilidade | Serve à pergunta da lente? | Colisão que resolve |
|---|-----------------|---------|-----------------|----------------------------|---------------------|
| 1 | **trait (nome)** | Trait que o item implementa/declara (`container_or_implemented_trait`) | **já tem / consulta fácil** | **Sim** — o trait é parte do contrato; distingue nós colidentes e permite nomeação honesta | `Display`+`Debug` (dois `fmt`); `HtmlElem::Type` (assoc types de traits distintos) |
| 2 | **trait_ref (com args)** | Referência completa do trait, com argumentos genéricos (`Impl::trait_ref` + `HirDisplay`) | **consulta fácil** (render barato) | **Sim** — completa o #1 nos casos em que o nome do trait coincide | `From<X>`+`From<Y>`; `Add<Self>`+`Add<f64>` (mesmo trait, args diferentes) |
| 3 | **kind: modificadores** | `const`/`async`/`unsafe` de fn; `unsafe` de trait, como booleanos | **já tem** (hoje achatado na string `kind`) | **Sim (parcial)** — `unsafe`/`async`/`const` são contrato de chamada; mudá-los quebra chamadores | — (não é colisão; é desachatamento) |
| 4 | **cfg** | Expressão `#[cfg(...)]` estruturada (`ItemCfgAttr`) | **já tem** (extração pronta) | **Sim (moderado)** — item gated só existe sob certas configs; raio condicional | — |
| 5 | **is_test** | Função marcada `#[test]` | **já tem** | **Sim (fraco)** — sinaliza que o raio é só-teste (mexer não quebra produção) | — |
| 6 | **macro_kind** | `fn-like` / `attr` / `derive` / `macro_rules!` (`Macro::kind`) | **consulta fácil** | **Sim (moderado)** — natureza da macro muda o que ela afeta | macros (padrão de colisão de macro citado na medição) |
| 7 | **is_non_exhaustive** | `#[non_exhaustive]` (`attrs.is_non_exhaustive`) | **consulta fácil** | **Sim** — é literalmente sobre raio entre-crates: muda o que quebra a jusante | — |
| 8 | **is_deprecated / is_unstable** | flags de estabilidade/intenção (`attrs`) | **consulta fácil** | **Sim (fraco)** — sinal de intenção/estabilidade, tangencia o raio | — |
| 9 | **signature** | Tipos de parâmetros + retorno renderizados (`params_without_self`, `ret_type`, `HirDisplay`) | **consulta cara** (render + estabilidade); tipos já em mãos | **Sim, mas redundante** — a assinatura é o contrato, mas arestas `uses` já codificam os tipos | — |
| 10 | **generics + bounds** | Parâmetros genéricos e seus bounds (`GenericDef::params`, `trait_bounds`) | **consulta cara** | **Sim (moderado)** — bounds definem o que o item exige; mudá-los quebra impls/chamadores | — |
| 11 | **lifetimes** | Parâmetros de lifetime (`GenericDef::lifetime_params`) | **consulta cara** | **Duvidoso** — raramente muda o "o que quebra" no grão do grafo-de-módulos | — |
| 12 | **is_doc_hidden / docstring** | `#[doc(hidden)]` e texto da doc (`is_doc_hidden`, `hir_docs`) | **consulta fácil** | **Duvidoso** — descrição humana, não raio de impacto (`doc_hidden` tem leve sinal de API pública vs interna) | — |
| 13 | **derive (nomes)** | Lista de traits derivados | **exposto, mas indireto** — não é atributo iterável aqui; surge como **impls expandidos** (#1) sob `--sysroot` | **Sim**, via #1 | coberto por #1 + nós de impl expandidos |
| 14 | **source span** | Arquivo + linha/coluna do item (`HasSource` + VFS + `LineIndex`) | **consulta cara** (mais plumbing: VFS + LineIndex) | **Não** para a lente — metadado de localização. **Mas** serve a uma feature futura de "abrir no editor" | — |
| 15 | **module_path / contêiner** | Módulo-pai do item | **já tem / consulta fácil** | **Redundante** com `path` (que já é qualificado) | — |

---

## 4. Agrupamentos (só para leitura — não é escolha)

Sem decidir nada, três grupos ajudam a ler a tabela:

**Grupo A — baratos e que servem ao raio de impacto** (faixa "já tem" ou
"consulta fácil", filtro "Sim"):
`trait (nome)` (#1), `trait_ref com args` (#2), `kind: modificadores` (#3),
`cfg` (#4), `macro_kind` (#6), `is_non_exhaustive` (#7). O #1+#2 juntos cobrem
**todos os padrões de colisão medidos** que envolvem traits
(`Display+Debug`, `From<X>+From<Y>`, `Add<Self>+Add<f64>`, `HtmlElem::Type`); o
#6 cobre o padrão de macro. O #13 (derive) cai aqui via #1, desde que com
`--sysroot`.

**Grupo B — caros, servem mas com ressalva** (faixa "consulta cara", filtro
"Sim/parcial/redundante"):
`signature` (#9, redundante com `uses`), `generics + bounds` (#10),
`is_deprecated/is_unstable` (#8, fáceis mas sinal fraco).

**Grupo C — caros e/ou de utilidade duvidosa para a lente**:
`lifetimes` (#11), `doc_hidden/docstring` (#12), `source span` (#14 — não serve
ao raio, mas é o único candidato com valor claro para uma feature futura de
navegação no editor), `module_path` (#15, redundante).

---

## 5. Observações de fechamento

- **O campo que motivou a investigação é o mais barato do conjunto.** O trait
  (#1) é "já tem": o builder já tinha o `impl` na mão e o descartava, e o
  `Printer` consegue reconsultá-lo numa linha. Distingue `Display`/`Debug` e
  os associated types `HtmlElem::Type` sem mais nada.
- **Uma armadilha concreta a registrar para a fase de escolha:** o trait **por
  nome** (#1) **não basta** para `From<X>`/`From<Y>` nem `Add<Self>`/`Add<f64>`
  — esses precisam dos **argumentos genéricos do trait** (#2, `trait_ref`).
  Quem escolher "emitir o trait" deve decidir explicitamente entre nome e
  referência-com-args, porque os dois resolvem conjuntos diferentes das
  colisões medidas.
- **"Derive" não é um atributo legível neste RA.** É impl expandido + trait, e
  só com `--sysroot`. Tratar derive como um campo de string por item seria ir
  contra como o rust-analyzer (e o próprio fork) já representa a informação.
- **Span é o único candidato cujo valor mora fora da lente.** Não serve ao raio
  de impacto, mas é o gancho de uma feature futura de navegação — registrado,
  não descartado, conforme o filtro pediu.

A escolha de quais destes entram no descritor é do autor. Esta investigação só
organizou os candidatos com disponibilidade, custo e o padrão de colisão que
cada um resolve.
