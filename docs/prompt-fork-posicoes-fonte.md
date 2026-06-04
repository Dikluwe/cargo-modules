# Prompt: Posições no fonte (arquivo + faixa) por nó no JSON do Fork

**Destino**: repositório do fork — https://github.com/Dikluwe/cargo-modules
(MPL-2.0; rust ≥ 1.91, edition 2024)
**Tipo**: modificação do fork (quinta rodada — após export-json, identidade-por-nó,
descritor semântico, subtipos de `uses`)
**Criado em**: 2026-06-03
**Decisões de origem**: trilha local da lente — mostrar **o que uma mudança
toca** antes do agente executar; **leitura do fork** confirmando o custo:
`analyzer::module_file` já resolve o arquivo de um módulo a partir do `hir` + vfs;
o vfs já é carregado no despacho (`src/command.rs:76`) e já é passado a um
subcomando irmão (`orphans`, `src/command.rs:87`). Decisão do autor de aproveitar
a ida ao fork para incluir as posições.

---

## Contexto

O fork emite, por nó, `id`/`path`/`name`/`kind`/`visibility` + descritor
semântico. O que **falta** para a trilha local é a **posição no fonte** de cada
item: o arquivo e a faixa de linhas onde ele é declarado. Sem isso, não há como
ligar um diff (arquivos + linhas alteradas) aos nós do grafo; com isso, dá para
pegar os nós cujo intervalo contém as linhas mudadas e mostrar o raio de impacto
**antes** de o agente rodar o comando — a peça que destrava aquela ideia.

A informação existe no rust-analyzer e o fork já a usa para módulos:
`analyzer::module_file` (`src/analyzer.rs:744`) faz
`module.definition_source(db)` → `.file_id.original_file(db)` →
`vfs.file_path(...)` → caminho. Este prompt generaliza isso de "arquivo do
módulo" para "arquivo + faixa de **cada item**", e emite no JSON.

---

## Por que esta rodada é barata e de baixo risco (diferente dos subtipos)

A quarta rodada (subtipos de `uses`) mexeu no **modelo do grafo** — enum
`Relationship`, construtor, chave de deduplicação — e foi a chave de dedup
compartilhada que gerou o efeito colateral no `dependencies` (§5 daquele prompt).

**Esta rodada não toca nada disso.** Posição é uma propriedade **por nó**,
derivável da fonte do `node.hir` no momento da serialização — exatamente o tier
do descritor semântico (terceira rodada, só printer). A única diferença para o
descritor é que resolver o **caminho** precisa do **vfs**, e o vfs hoje não chega
ao `Printer` do `export_json`. Mas threadá-lo é a **mesma adição de um argumento
que o `orphans` já faz** (`src/command.rs:87`): não é encanamento novo.

Não há toque no enum, no construtor, nem na chave de dedup. Logo o tipo de
acoplamento escondido que mordeu os subtipos **não existe** aqui.

---

## Restrições

- **Aditivo e retrocompatível.** Todos os campos existentes permanecem. A posição
  é um campo **novo**, **opcional** (ausente quando o item não tem fonte de
  arquivo). Quem lê o schema antigo continua funcionando.
- **Mexe em**: o despacho (`src/command.rs`, uma linha), o comando do
  `export_json` (`src/command/export_json/command.rs`, assinatura do `run`) e o
  printer (`src/command/export_json/printer.rs`). **Não toca o modelo do grafo**
  (`graph.rs`), o **construtor** (`builder.rs`), nem a chave de dedup.
- **Não muda o comportamento** de outros subcomandos. O `orphans` já recebe vfs;
  `structure`/`dependencies` não são afetados.
- **Sem ruído em stdout.** JSON parseável direto.
- **Versão**: aditiva → bump minor ou patch, não major.
- **Posição é por nó (item)**, não por aresta. (Mapear um diff aos itens basta
  para a trilha local; onde uma referência específica acontece é granularidade
  mais fina e não é necessária aqui.)

---

## A mudança

### 1. Threading do vfs até o printer

- `src/command.rs:89` — hoje `Self::ExportJson(command) => command.run(krate, db,
  edition)`. Passar o vfs, **copiando a linha do `orphans` logo acima** (`:87`):
  `command.run(krate, db, &vfs, edition)`.
- `src/command/export_json/command.rs` — a assinatura do `run` ganha
  `vfs: &vfs::Vfs` (igual à de `src/command/orphans/command.rs:36`); repassar ao
  `Printer::new`.
- `src/command/export_json/printer.rs` — `Printer` ganha o campo
  `vfs: &'a vfs::Vfs`; `Printer::new` recebe e armazena.

### 2. Posição por nó no printer

`JsonNode` ganha um campo opcional. Sugestão (objeto aninhado, nomes a
confirmar):

```rust
#[derive(Serialize)]
struct JsonSpan {
    file: String,        // caminho do arquivo (absoluto, como module_file devolve)
    start_line: u32,     // 1-based
    end_line: u32,
    // start_col / end_col: refinamento opcional
}

// em JsonNode:
#[serde(skip_serializing_if = "Option::is_none")]
position: Option<JsonSpan>,
```

Obter a posição de um `node.hir` (`hir::ModuleDef`), modelado no `module_file`
mas generalizado para qualquer item:

- A fonte do item dá um `InFile<…>` com `file_id` e o nó de sintaxe; daí
  `text_range()` (faixa em **bytes**) e `file_id.original_file(db)` → `FileId`
  (mapeia macro-expansão de volta ao arquivo do call-site, como o `module_file`
  já faz com `original_file`).
- Caminho: `vfs.file_path(file_id…)` → caminho (mesma resolução do
  `module_file`).
- **Linhas**: converter os offsets de `text_range` em número de linha via o
  `LineIndex` do arquivo (consulta do db/ide; uma por arquivo, cacheável). Emitir
  `start_line`/`end_line` (1-based). (Alternativa, se preferir adiar a conversão:
  emitir os offsets em bytes crus e deixar a lente converter — **mas** o uso é
  mapear diff, que é por linha, então emitir linha aqui poupa a lente; recomendo
  emitir linha.)
- **Ausência**: itens sem fonte de arquivo (tipos embutidos, p.ex.) → `position`
  ausente (como `module_file` devolve `None`). Idem se a resolução do caminho
  falhar — não emitir, não abortar.
- Atualizar o doc-comment do schema no topo do arquivo.

Reusar o padrão vfs→caminho do `module_file`: ou fatorar um helper em
`analyzer.rs` (`item_source_span`/`item_file`) que `module_file` e o printer
compartilham na parte de resolução, ou método no `Printer` no estilo dos
`descriptor_*`. Decisão do executor; o ponto é não duplicar a lógica do
`module_file`.

---

## Verificação

- Os testes existentes do fork passam; atualizar o snapshot do export-json para
  incluir o campo `position` (aditivo, opcional).
- **Smoke test** num crate real: um nó de item conhecido (ex.: uma struct num
  arquivo conhecido) sai com `position.file` correto e `start_line`/`end_line`
  batendo com a declaração no fonte.
- **Itens sem fonte** (tipo embutido) → sem `position`.
- **Item gerado por macro** → `position` no **call-site** (via `original_file`),
  não num arquivo sintético de expansão.
- **Determinismo**: duas extrações do mesmo crate produzem JSON idêntico.
- Campos existentes inalterados; `position` é aditivo-opcional — consumidores
  antigos intactos.
- Outros subcomandos (`structure`/`dependencies`/`orphans`) compilam e se
  comportam **identicamente** (esta rodada só toca o despacho do `export_json` e
  o seu printer; o `orphans` já tinha vfs).

---

## O que este prompt NÃO faz

- **Não toca o modelo do grafo** (enum/construtor/dedup) — ao contrário dos
  subtipos. Sem ripple para outros subcomandos.
- **Não toca a lente.** Consumir as posições — mapear um diff aos nós e mostrar o
  raio antes do agente executar — é prompt próprio **no repositório da lente**,
  na trilha local.
- **Posição por aresta**: fora de escopo (posição por nó basta para mapear diff).
- **Relativizar o caminho**: o fork emite o caminho como o `module_file` o tem
  (absoluto); relativizar à raiz do crate, se preciso, é trabalho da lente ao
  consumir.
- **Não cria grafo de chamadas comportamental**: continua estrutura, não fluxo. O
  Limite 3 segue.

---

## O ganho

Com arquivo + faixa de linhas por nó, a lente passa a poder mapear um diff
(arquivos e linhas alteradas) aos nós cujo intervalo contém aquelas linhas, e
mostrar o raio de impacto desses nós **antes** de o agente rodar o comando — a
trilha local que, sem posição, não saía do papel. É a peça habilitante; o que a
lente faz com ela é a próxima decisão, do lado da lente.

---

## Histórico de Revisões

| Data | Motivo | Arquivos |
|------|--------|----------|
| 2026-06-03 | Posição no fonte (arquivo + faixa de linhas) por nó no export-json. vfs threadado até o `Printer` copiando o padrão do `orphans` (`command.rs:89` ganha `&vfs`; assinatura do `run` do export_json); `JsonNode` ganha `position` opcional, derivada da fonte do `node.hir` no estilo generalizado do `module_file` (caminho via vfs; linhas via LineIndex; ausente para itens sem fonte; call-site para macro-gerados). **Não toca o modelo do grafo** (enum/construtor/dedup), ao contrário dos subtipos — baixo risco. Aditivo e retrocompatível. Não toca a lente, nem posição por aresta, nem grafo de chamadas. | `src/command.rs`, `src/command/export_json/command.rs`, `src/command/export_json/printer.rs`, (opcional `src/analyzer.rs` para helper), `tests/export_json.rs`, `docs/prompt-fork-posicoes-fonte.md` |
| 2026-06-03 | Implementação. Helper `analyzer::item_source_span` (+ `ItemSourceSpan`) generaliza `module_file`: usa `HasSource`/`definition_source` por variante de `ModuleDef`, `InFile::syntax().original_file_range_rooted` (mapeia macro→call-site para arquivo **e** faixa numa só chamada), `EditionedFileId::file_id` para o vfs path e `LineIndexDatabase::line_index` para linhas 1-based. O `Printer` passou a guardar `&RootDatabase` (não só `&dyn HirDatabase`) porque `line_index` exige `LineIndexDatabase`; vfs threadado via `command.rs:89` e assinatura do `run`. `JsonNode.position` ({`file`,`start_line`,`end_line`}). Como bônus, separa nós de `path` colidente por linha. Testes novos `node_position_is_file_and_line_range` e `position_distinguishes_colliding_path_nodes`; suíte verde (236). **Nota**: `position` é determinístico; a variação restante entre extrações é só o `id` do petgraph, instabilidade **pré-existente** já documentada no schema (não introduzida aqui — não tocou criação de nós). | `src/analyzer.rs`, `src/command.rs`, `src/command/export_json/command.rs`, `src/command/export_json/printer.rs`, `tests/export_json.rs`, `docs/prompt-fork-posicoes-fonte.md` |
