# Prompt para Claude Code — Subcomando de exportação JSON no fork do cargo-modules

> **Onde usar este prompt**: dentro do repositório do *fork* do
> `cargo-modules` (projeto externo, MPL-2.0). **Não** é um prompt do
> projeto-lente nem vai para `00_nucleo/`. É instrução de trabalho para uma
> ferramenta externa, conforme o ADR-0001.
>
> **Contexto de origem**: a forma do JSON definida abaixo é a *forma
> organizada* da Lente de Forma e Consequência (passo 2 da proposta). Ela é o
> contrato que o L1 da lente vai consumir e que o L3 vai ler. O printer deve
> emiti-la fielmente. Não altere os nomes de campos sem que essa decisão seja
> refletida do lado da lente.

---

## Objetivo

Adicionar ao fork do `cargo-modules` um **novo subcomando** que percorre o
mesmo grafo interno que o subcomando `dependencies` usa e o serializa em
**JSON estruturado**, em vez de DOT. O subcomando deve emitir uma tradução
**fiel** do grafo: todos os dados que a ferramenta já extrai de cada nó e
cada aresta, sem descartar nenhum.

O DOT existente é destinado a renderização visual e não é consumível com
robustez por outro programa. O JSON deste subcomando é a fonte de dados da
lente.

---

## Restrições

- **Não modifique** o pipeline existente (`structure`, `dependencies`,
  `orphans`). Adicione um subcomando novo, lado a lado, reutilizando a
  infraestrutura de grafo já existente.
- **Reutilize o `GraphBuilder`**. Não reconstrua a lógica de varredura do
  grafo. O subcomando `dependencies` constrói o grafo assim (em
  `src/command/dependencies/command.rs`):
  ```rust
  let builder = GraphBuilder::new(db, edition, krate);
  let (graph, crate_node_idx) = builder.build()?;
  ```
  O novo subcomando deve fazer o mesmo build e então serializar `graph`
  (`petgraph::stable_graph::StableGraph<Node, Edge>`) em JSON.
- **Siga o padrão de subcomando do projeto**. Cada subcomando é uma variante
  do enum `Command` em `src/command.rs`, com seu próprio módulo contendo
  `command.rs`, `options.rs` e o equivalente a `printer.rs`. Crie um módulo
  novo no mesmo formato (sugestão de nome: `export_json` ou `json`).
- **Resolva os dados dentro do subcomando**, onde o `db: &dyn HirDatabase` e
  o `edition` estão disponíveis. Cada `Node` (`Item`) só resolve seus dados
  via métodos que exigem o `db`; um consumidor externo não conseguiria. Por
  isso a serialização tem de acontecer aqui.
- **Licença**: arquivos novos herdam o cabeçalho MPL-2.0 dos demais arquivos
  do projeto. Mantenha o cabeçalho.

---

## A forma do JSON (contrato — emitir exatamente esta estrutura)

```json
{
  "crate": "<nome do crate raiz>",
  "nodes": [
    {
      "path": "meu_crate::modulo::item",
      "name": "item",
      "kind": "fn",
      "visibility": "pub"
    }
  ],
  "edges": [
    {
      "from": "meu_crate::modulo::item_a",
      "to": "meu_crate::modulo::item_b",
      "relation": "uses"
    }
  ]
}
```

### Campos de cada nó (`nodes[]`)

| Campo | Origem no código | Valores |
|-------|------------------|---------|
| `path` | `Item::display_path(db, edition)` | caminho canônico; **identidade única** do nó |
| `name` | `Item::display_name(db, edition)` | nome curto do item |
| `kind` | `ItemKindDisplayName` (`src/item/kind_display_name.rs`) | um de: `crate`, `mod`, `fn`, `const fn`, `async fn`, `unsafe fn`, `struct`, `union`, `enum`, `variant`, `const`, `static`, `trait`, `unsafe trait`, `type`, `builtin`, `macro` |
| `visibility` | `ItemVisibility` (`src/item/visibility.rs`) | `pub`, `pub(crate)`, `pub(in crate::<path>)`, `pub(super)`, ou `priv` (privado) |

### Campos de cada aresta (`edges[]`)

| Campo | Origem no código | Valores |
|-------|------------------|---------|
| `from` | `display_path` do nó de origem | caminho canônico (referencia um `path` de `nodes`) |
| `to` | `display_path` do nó de destino | caminho canônico (referencia um `path` de `nodes`) |
| `relation` | `Relationship` (`src/graph.rs`) | `owns` (contenção: módulo contém item) ou `uses` (uso: item usa outro) |

Notas de fidelidade:
- As arestas são **dirigidas**: `from` → `to`. Preserve a direção tal como o
  grafo a tem.
- Emita **todos** os nós e **todas** as arestas do grafo construído, das duas
  relações (`owns` e `uses`). Não filtre por padrão; se quiser oferecer os
  mesmos filtros do `dependencies` (`--no-fns`, `--no-uses`, etc.), que seja
  por flag opcional, com o padrão sendo emitir tudo.
- A serialização pode usar `serde`/`serde_json` (adicione ao `Cargo.toml` do
  fork) ou a lib `json` já presente. `serde_json` é preferível pela robustez.

---

## Acionamento

Novo subcomando, invocável como:
```
cargo modules <nome-do-subcomando>   # ex: cargo modules export-json
```
Aceita as mesmas opções de seleção de alvo que os outros subcomandos
(`--lib`, `--bin`, `-p/--package`, `--manifest-path`, `--cfg-test`,
features), porque precisa carregar o workspace do mesmo jeito. Emite o JSON
em stdout.

---

## Testes (obrigatório)

Gere testes junto com o subcomando. O fork já tem `tests/` com projetos de
exemplo (`tests/projects/`). Use um crate pequeno conhecido (por exemplo o
`tests/projects/smoke` ou o `readme_graph_example`) como entrada e verifique:

1. **Estrutura do JSON**: a saída é JSON válido com as chaves `crate`,
   `nodes`, `edges`.
2. **Fidelidade dos nós**: para o crate de exemplo, os nós esperados estão
   presentes, com `path`, `name`, `kind` e `visibility` corretos para ao
   menos alguns itens conhecidos (ex: uma `fn pub`, um `mod pub(crate)`).
3. **Fidelidade das arestas**: ao menos uma aresta `owns` (módulo contém
   item) e uma `uses` (item usa outro) aparecem com `from`/`to`/`relation`
   corretos.
4. **Direção**: uma aresta conhecida tem `from` e `to` na ordem certa.
5. **Cobertura**: a contagem de nós/arestas do JSON bate com a do grafo
   construído (nada é silenciosamente descartado).

---

## Resultado esperado

- Um módulo de subcomando novo (`src/command/<nome>/` com `command.rs`,
  `options.rs`, e um printer JSON), registrado como variante no enum
  `Command` de `src/command.rs`.
- O subcomando produz, em stdout, o JSON na forma especificada acima.
- Testes cobrindo os cinco pontos acima, passando.
- Nenhuma modificação nos subcomandos existentes.

Ao terminar, mostre um exemplo real da saída JSON rodada contra um dos
projetos de teste, para conferência da forma.
