# Prompt para Claude Code — Fechamento do fork do cargo-modules

> **Onde usar**: dentro do repositório do *fork* do `cargo-modules`, na sessão
> do Claude Code que implementou o `export-json`.
>
> **O que este prompt faz**: finaliza e verifica o trabalho, documenta a
> invocação, e deixa o repositório **pronto para push** — sem dar push. O push
> é feito pelo humano, depois.
>
> **O que este prompt NÃO faz**: não dá `git push`; não muda o default do
> `--sysroot` (o fork fica neutro, default desligado, por decisão de
> arquitetura — ver nota abaixo).

---

## Nota de arquitetura — não mexer no default do sysroot

O `--sysroot` permanece **opt-in** (default desligado), exatamente como está.
Isto é decisão deliberada: o fork é uma ferramenta neutra que oferece a
capacidade; quem decide usá-la com sysroot é o projeto que consome o fork (a
camada L3 da lente invoca sempre com `--sysroot`). **Não inverta o default, não
adicione lógica que force sysroot.** O fork expõe a opção; a política de uso
mora fora dele.

---

## Tarefas, em ordem

### 1. Verificar que nada ficou pela metade

- Confirme que `cargo build` e `cargo build --release` compilam sem erro nem
  warning novo introduzido pelo `export-json`.
- Confirme que a suíte passa: `cargo test`. Reporte o resultado (passed/failed
  por arquivo de teste).
- Confirme que o working tree não tem arquivos temporários ou de depuração
  esquecidos (ex: `/tmp/*.json` referenciados, prints de debug, código
  comentado). Se houver, limpe.

### 2. Verificar o binário standalone (release, fora do diretório do fork)

Este é o uso real: a camada L3 da lente vai invocar o binário de **outro**
diretório, contra **outros** crates. Confirme que isso funciona:

- Compile em release: `cargo build --release`.
- Localize o binário gerado (`target/release/cargo-modules`).
- Rode-o contra um crate Rust **fora** do diretório do fork — pode ser um crate
  pequeno qualquer que exista na máquina, ou crie um crate mínimo temporário
  para o teste. Invoque:
  ```
  <caminho>/target/release/cargo-modules modules export-json --sysroot --compact
  ```
  (ou a forma de invocação correta do binário como subcomando cargo)
- Confirme que a saída é o JSON esperado (chaves `crate`, `nodes`, `edges`) e
  reporte a contagem de nós/arestas obtida, para evidência.
- Se a invocação standalone exigir alguma variável de ambiente, toolchain
  específica, ou passo de instalação (`cargo install --path .`), **documente
  isso** — é informação que a L3 vai precisar.

### 3. Documentar a invocação no README do fork

Adicione ao `README.md` do fork uma seção curta sobre o `export-json`, contendo:

- O propósito em uma linha: exporta o grafo interno do crate como JSON
  estruturado, para consumo por outras ferramentas.
- A invocação canônica:
  ```
  cargo modules export-json --sysroot --compact
  ```
- O significado das flags relevantes: `--sysroot` (inclui impls de derives e
  itens da stdlib no grafo — necessário para fidelidade; sem ele o grafo perde
  as relações criadas por `#[derive(...)]`), `--compact` (JSON em uma linha).
- A forma do JSON de saída (estrutura `crate` / `nodes[]` / `edges[]` com os
  campos de cada um). Pode referenciar a tabela que já está no relatório de
  implementação.
- Uma nota de que o `--sysroot` é opt-in por design (o fork é neutro; o
  consumidor decide).

### 4. Preparar o commit — e PARAR

- Adicione os arquivos relevantes (`git add`).
- Crie um commit com mensagem descritiva. Sugestão de mensagem (ajuste se
  preferir):
  ```
  feat: subcomando export-json — exporta grafo interno como JSON estruturado

  Adiciona o subcomando `export-json` que serializa o grafo de
  Item/Relationship (o mesmo usado por `dependencies`) em JSON, com nós
  (path, name, kind, visibility) e arestas dirigidas (from, to, relation
  owns/uses). Saída determinística. --sysroot opt-in para incluir derives;
  --compact para JSON em uma linha. Sem regressão nos subcomandos existentes.
  ```
- **PARE AQUI.** Não execute `git push`. Deixe o working tree limpo, o commit
  feito, e reporte:
  - o hash do commit;
  - o branch atual;
  - o resultado dos testes;
  - a contagem de nós/arestas do teste standalone;
  - qualquer passo de instalação que a L3 vá precisar (do item 2).

Ao terminar, diga explicitamente: "Pronto para push. Working tree limpo,
commit X no branch Y. Execute `git push` quando quiser subir."

---

## Resultado esperado

- Fork compilando em debug e release, testes passando.
- Binário release verificado rodando standalone, fora do diretório do fork.
- README do fork documenta o `export-json` e sua invocação.
- Um commit pronto, working tree limpo, **sem push**.
- Relatório final com hash, branch, testes, contagem do teste standalone, e
  eventual passo de instalação para a L3.
