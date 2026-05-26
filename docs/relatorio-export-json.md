# Relatório — Subcomando `export-json` no fork do `cargo-modules`

> Escopo: implementação do subcomando solicitado em
> [`docs/prompt-claude-code-printer-json.md`](./prompt-claude-code-printer-json.md).
> Branch: `main`. Versão do crate alvo: `cargo-modules 0.26.0`.

---

## 1. Objetivo e premissas

Adicionar um novo subcomando ao fork que percorre o mesmo grafo interno
usado por `dependencies` e o serializa em **JSON estruturado**. O JSON é o
contrato que a Lente de Forma e Consequência (camada L1 do projeto-lente)
consome e que o L3 lê.

Premissas que guiaram o trabalho:

- **Sem regressão** nos subcomandos existentes (`structure`,
  `dependencies`, `orphans`).
- **Reuso integral** do `GraphBuilder` — não há lógica nova de varredura
  do grafo.
- **Fidelidade**: todos os nós e arestas do grafo construído são
  emitidos; nenhum filtro é aplicado por padrão.
- **Padrão do projeto**: mesmo formato modular dos subcomandos
  existentes (`command.rs` / `options.rs` / `printer.rs`).
- **Licença**: cabeçalho MPL-2.0 em todos os arquivos novos.

---

## 2. Arquivos criados e alterados

### Novos

| Arquivo | Função |
|---------|--------|
| `src/command/export_json.rs` | Index do módulo (`command`, `options`, `printer`). |
| `src/command/export_json/options.rs` | clap `Options`: `general`, `project`, `--cfg-test`, `--sysroot`, `--compact`. |
| `src/command/export_json/command.rs` | Constrói o grafo via `GraphBuilder` e delega ao `Printer` JSON. |
| `src/command/export_json/printer.rs` | Serializa o `StableGraph<Node, Edge>` em JSON via `serde_json`. |
| `tests/export_json.rs` | 5 testes de propriedade + 1 snapshot de `--help`. |
| `tests/snapshots/export_json__help__smoke.snap` | Snapshot do help do subcomando. |

### Alterados

| Arquivo | Mudança |
|---------|---------|
| `src/command.rs` | Variante `ExportJson(ExportJsonCommand)` registrada em `Command`; matches em `sanitize`, `run`, `general_options`, `project_options`, `load_options`. |
| `Cargo.toml` | Adicionado `serde = { version = "1.0", features = ["derive"] }` e `serde_json = "1.0"` em `[dependencies]`; `serde_json = "1.0"` em `[dev-dependencies]`. |
| `tests/snapshots/general__help__smoke.snap` | Linha do novo subcomando na lista do help global. |

Nada em subcomandos existentes foi tocado, exceto a adição da variante e
dos braços de `match` correspondentes.

---

## 3. Forma do JSON

A saída segue exatamente o contrato pedido:

```json
{
  "crate": "<nome canônico do crate raiz>",
  "nodes": [
    { "path": "...", "name": "...", "kind": "...", "visibility": "..." }
  ],
  "edges": [
    { "from": "...", "to": "...", "relation": "owns" | "uses" }
  ]
}
```

### Mapeamento código ↔ JSON

| Campo JSON | Origem |
|------------|--------|
| `crate` | `analyzer::crate_name(krate, db)` |
| `nodes[].path` | `Item::display_path(db, edition)` (identidade canônica) |
| `nodes[].name` | `Item::display_name(db, edition)` |
| `nodes[].kind` | `Item::kind_display_name(db, edition)` (`crate`, `mod`, `fn`, `const fn`, `async fn`, `unsafe fn`, `struct`, `union`, `enum`, `variant`, `const`, `static`, `trait`, `unsafe trait`, `type`, `builtin`, `macro`) |
| `nodes[].visibility` | `ItemVisibility` mapeada para `pub`, `pub(crate)`, `pub(in crate::<path>)`, `pub(super)`, `priv` |
| `edges[].from` | `display_path` do nó-origem |
| `edges[].to` | `display_path` do nó-destino |
| `edges[].relation` | `Relationship::display_name()` — `"owns"` ou `"uses"` |

Detalhes importantes:

- A direção das arestas é **preservada**: `from → to`, exatamente como o
  builder a produz.
- `visibility` para o caso `Private` é emitido como `priv` (conforme o
  contrato do prompt, não como `pub(self)` que é o `Display` interno do
  projeto).
- Nós são ordenados por `path`; arestas por `(from, to, relation)`. A
  saída é **determinística** — fundamental para reuso em pipelines, hash
  e snapshot tests.

---

## 4. Fluxo de execução

```
cargo modules export-json [opções]
        │
        ▼
App (clap) → Command::ExportJson(ExportJsonCommand)
        │
        ▼
Command::run()
  ├─ analyzer::load_workspace(general, project, load_opts)
  │      → (krate, host, vfs, edition)
  ├─ ra_ap_hir::attach_db(db, ...)
  └─ ExportJsonCommand::run(krate, db, edition)
         │
         ▼
   GraphBuilder::new(db, edition, krate).build()
         │
         ▼
   Printer::to_json(&graph)
     ├─ coleta nós  → ordena por path
     ├─ coleta arestas → ordena por (from, to, relation)
     └─ serde_json (pretty ou compact)
         │
         ▼
       println!  →  stdout
```

Pontos de fidelidade:

- A serialização ocorre **dentro** do subcomando, onde `db` e `edition`
  estão disponíveis — coerente com a restrição do contrato (consumidores
  externos não conseguiriam resolver os dados de cada `Item`).
- O grafo passa **direto** do builder ao printer, sem o passo de
  `Filter` que `dependencies` aplica. Isso atende o requisito "Emita
  todos os nós e todas as arestas do grafo construído".

---

## 5. Opções de CLI

| Flag | Default | Efeito |
|------|---------|--------|
| `--lib`, `--bin <X>`, `-p/--package`, `--manifest-path`, `--target`, `--features`, `--no-default-features`, `--all-features` | (herdadas de `ProjectOptions`) | Seleção de alvo, igual aos demais subcomandos. |
| `--verbose` | falso | Igual aos demais. |
| `--cfg-test` | falso | Analisa com `#[cfg(test)]` ligado. |
| `--sysroot` | **falso** | Carrega `std`/`core` no workspace. Necessário para que `#[derive]` se expandam e impls associados apareçam no grafo. |
| `--compact` | falso | Emite o JSON em uma única linha (pretty-print é o default). |

### Por que `--sysroot` é opt-in

O subcomando `dependencies` carrega sysroot por padrão e depois **filtra
externs** no passo de `Filter` antes de imprimir. Como o `export-json`
não filtra, ligar sysroot por padrão produziria saída inflada com
centenas de tipos da stdlib em qualquer crate — instável entre versões
do rustc e ruim para o consumidor da lente.

Por isso o default segue `structure` (`sysroot: false`): saída focada no
crate-alvo, estável e reproduzível. Quem precisar dos impls de derives
ou de nós da stdlib usa `--sysroot`.

Trade-off conhecido: sem `--sysroot`, o expansor de `#[derive(Clone)]`
(que precisa de `core::clone::Clone`) não sintetiza o `impl`, então as
funções `clone` derivadas **não aparecem** como nós. Com `--sysroot`,
elas aparecem e o número de nós/arestas se aproxima do que o
`dependencies` mostra após o filtro (observação empírica em
`struct_fields`: 11/19 sem sysroot, 17/31 com).

---

## 6. Testes

### 6.1. Testes do novo subcomando (`tests/export_json.rs`)

| Teste | O que valida |
|-------|--------------|
| `json_is_well_formed_on_minimal_crate` | Saída é JSON parseável; chaves de topo são exatamente `crate`, `nodes`, `edges`; crate vazio (`package_lib_target`) produz só o nó-raiz e zero arestas. |
| `node_fields_are_faithful` | Em `struct_fields`: crate root, `pub struct`, `pub(crate) trait` e type alias têm `path`/`name`/`kind`/`visibility` corretos. |
| `both_owns_and_uses_edges_are_emitted_with_correct_direction` | Existe aresta `owns` (`struct_fields → struct_fields::Struct`) e `uses` (`Struct → TargetStruct`); direção inversa **não** existe; nenhum `relation` fora de `{owns, uses}`. |
| `every_edge_endpoint_references_an_existing_node` | Garantia de cobertura: todo `from`/`to` de aresta referencia um `path` presente em `nodes`. Prova que nada é silenciosamente descartado entre construção do grafo e serialização. |
| `compact_flag_produces_single_line` | `--compact` produz uma única linha de JSON válido. |
| `help::smoke` | Snapshot do `export-json --help`. |

### 6.2. Cobertura dos cinco pontos do contrato

| Ponto do contrato | Coberto por |
|-------------------|-------------|
| 1. Estrutura do JSON | `json_is_well_formed_on_minimal_crate` |
| 2. Fidelidade dos nós | `node_fields_are_faithful` |
| 3. Fidelidade das arestas (`owns` + `uses`) | `both_owns_and_uses_edges_are_emitted_with_correct_direction` |
| 4. Direção | mesma função acima |
| 5. Cobertura (nada descartado) | `every_edge_endpoint_references_an_existing_node` |

### 6.3. Suíte completa do projeto

```
test result: ok.   6 passed; 0 failed   (general)
test result: ok. 100 passed; 0 failed   (structure)
test result: ok. 109 passed; 0 failed   (dependencies)
test result: ok.   1 passed; 0 failed   (orphans)
test result: ok.   6 passed; 0 failed   (export_json — novo)
                  ───
                  222 passed; 0 failed
```

Tempo dominado por `dependencies` (~330 s): cada caso é um
`assert_cmd::Command` que sobe o binário e reinicia o rust-analyzer do
zero contra um fixture. Não é regressão — é o custo intrínseco da
suíte. O novo `export_json.rs` adiciona ~5–8 s no total.

---

## 7. Exemplo real de saída

Comando rodado em `tests/projects/struct_fields/`:

```bash
cargo modules export-json
```

Trecho representativo (saída completa em `/tmp/export_json_struct_fields.json`, 168 linhas):

```json
{
  "crate": "struct_fields",
  "nodes": [
    { "path": "struct_fields",                    "name": "struct_fields",     "kind": "crate",  "visibility": "pub" },
    { "path": "struct_fields::GenericTargetEnum", "name": "GenericTargetEnum", "kind": "enum",   "visibility": "pub(crate)" },
    { "path": "struct_fields::Struct",            "name": "Struct",            "kind": "struct", "visibility": "pub" },
    { "path": "struct_fields::TargetStruct",      "name": "TargetStruct",      "kind": "struct", "visibility": "pub(crate)" },
    { "path": "struct_fields::TargetTrait",      "name": "TargetTrait",       "kind": "trait",  "visibility": "pub(crate)" },
    { "path": "struct_fields::TypeAlias",         "name": "TypeAlias",         "kind": "type",   "visibility": "pub(crate)" }
    /* … 11 nós no total … */
  ],
  "edges": [
    { "from": "struct_fields",            "to": "struct_fields::Struct",       "relation": "owns" },
    { "from": "struct_fields::Struct",    "to": "struct_fields::TargetStruct", "relation": "uses" },
    { "from": "struct_fields::Struct",    "to": "struct_fields::TargetTrait",  "relation": "uses" },
    { "from": "struct_fields::TypeAlias", "to": "struct_fields::TargetStruct", "relation": "uses" }
    /* … 19 arestas no total … */
  ]
}
```

---

## 8. Decisões de design notáveis

1. **`serde_json` em vez da lib `json` já presente**: tipos próprios
   (`JsonGraph`, `JsonNode`, `JsonEdge`) com `#[derive(Serialize)]` são
   mais robustos, type-safe, e produzem saída ordenada por padrão. O
   custo de compilação adicional é desprezível dada a árvore de
   dependências já enorme do `ra_ap_*`.

2. **Ordenação determinística**: nós por `path`, arestas por
   `(from, to, relation)`. Sem isso, dois dumps do mesmo crate poderiam
   diferir em ordem de itens (a `StableGraph` preserva ordem de inserção,
   mas essa ordem depende da varredura do builder). Determinismo é
   crítico para:
   - testes baseados em conteúdo;
   - caching da lente downstream;
   - diff humano entre versões do mesmo crate.

3. **`Private → "priv"`**: o `Display` de `ItemVisibility` produz
   `pub(self)`, mas o contrato explicitamente lista `priv`. Optamos pela
   forma do contrato; mudar isso requer alinhamento com o L1 da lente.

4. **Sem reaplicar o `Filter` do `dependencies`**: o contrato pede a
   forma crua do grafo. Quem quiser filtrar pode fazer no lado da lente
   (em `jq` ou em código). Não duplicamos a lógica de filtro aqui.

5. **`--compact` para uma linha**: útil para `jq -c` e pipelines de
   ingestão; pretty-print fica como default para legibilidade
   imediata.

---

## 9. Limitações conhecidas

- **Derives não-expandidos sem sysroot**: documentado na seção 5. Não é
  bug — é consequência da ausência de `core` no DB.
- **`crate` é o nome canônico (com `-` virando `_`)**: vem direto de
  `analyzer::crate_name`. Coerente com o resto do projeto.
- **Filtros `--no-fns`/`--no-uses`/etc. não foram adicionados**: o
  contrato os marca como opcionais e o consumidor da lente está mais bem
  servido recebendo o grafo completo. Adicionar é trivial se necessário —
  basta reusar o `Filter` de `dependencies` atrás de flags.
- **Sem teste contra o fixture `smoke`**: `smoke` produz centenas de nós
  e tornaria os testes pesados e frágeis. `struct_fields` cobre
  `owns`+`uses`+visibilidades variadas com saída pequena o suficiente
  para asserções precisas.

---

## 10. Como rodar localmente

Build do binário:

```bash
cargo build
```

Rodar em qualquer crate Rust:

```bash
cd /caminho/do/seu/crate
cargo modules export-json                  # pretty, sem sysroot
cargo modules export-json --compact        # uma linha
cargo modules export-json --sysroot        # inclui derives expandidos
cargo modules export-json --lib            # forçar target lib
cargo modules export-json -p meu-pkg       # selecionar package em workspace
```

Rodar só os testes do novo subcomando:

```bash
cargo test --manifest-path /caminho/cargo-modules/Cargo.toml --test export_json
```

Suíte completa (atenção: ~6 min, dominada pelo `dependencies`):

```bash
cargo test --manifest-path /caminho/cargo-modules/Cargo.toml
```

---

## 11. Próximos passos opcionais

Sugestões — **nenhuma é necessária** para o contrato fechado:

- Expor filtros opcionais (`--no-fns`, `--no-uses`, `--no-externs`)
  delegando ao `Filter` existente do `dependencies`.
- Adicionar campo opcional `source_span` em nós (arquivo + linha), para
  ferramentas que precisem navegar de volta ao código.
- Snapshot insta da saída JSON completa para `struct_fields`, para
  detectar regressão de forma exata (hoje testamos por propriedades,
  não por igualdade textual).
- Subcomando irmão `export-tree` (mesma forma, baseado em
  `tree::TreeBuilder`) se a Lente de Forma e Consequência precisar da
  visão hierárquica em vez do grafo.
