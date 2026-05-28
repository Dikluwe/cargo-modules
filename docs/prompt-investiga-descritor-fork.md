# Prompt: Investigação do Descritor Semântico Disponível no Fork

**Tipo**: Experimento de Arena (`lab/`) / investigação no fork
**Camada**: bancada — sem linhagem obrigatória. Resultado é insumo para o
autor decidir quais campos entram no descritor semântico do fork.
**Criado em**: 2026-05-28
**Decisões de origem**: laudo 0010 (D4 — imprecisão trait↔id); investigação do
elo trait↔id (`lab/investiga-elo-trait-id/relatorio.md`, concluiu que a
correção mora no fork); decisão do autor de fazer reforma estrutural (emitir
descritor rico) em vez de correção pontual.
**Destino da leitura**: repositório do fork
(https://github.com/Dikluwe/cargo-modules) e a API do rust-analyzer que ele
usa.

---

## Propósito

O autor decidiu que o fork passará a emitir um **descritor semântico mais
rico** por item, de uma vez, em vez de adicionar campo-a-campo a cada
necessidade (já foram dois: `id`, e agora o trait). Mas "rico" precisa virar
uma **lista concreta** de campos, escolhida com critério — não "tudo que o
rust-analyzer sabe" (over-engineering), não um chute.

Esta investigação **mapeia o que está disponível** para o autor escolher.
Ela **não decide** o descritor e **não modifica** o fork — só levanta os
candidatos com a informação necessária para a escolha.

---

## A pergunta-âncora (filtro contra over-engineering)

Para cada campo candidato, a pergunta que decide se ele vale: **ele ajuda a
responder a pergunta central da lente — "o que quebra se eu mexer aqui" — ou
é informação interessante mas irrelevante para o raio de impacto?**

Um campo que não serve a nenhuma pergunta plausível da lente não deve entrar,
por mais disponível que seja. Exemplos do filtro:

- `trait_impl` (o trait que um método implementa): **serve** — distingue nós
  colidentes e permite nomeação honesta. É o que motivou a investigação.
- span exato (linha/coluna do item no arquivo): provavelmente **não serve** à
  pergunta do raio — é metadado de localização, não de impacto. (Mas pode
  servir a uma feature futura de "abrir no editor" — a investigação deve
  notar isso, não descartar peremptoriamente.)

A investigação aplica esse filtro a cada candidato e classifica.

---

## O que investigar

### Parte 1 — O que o `cargo-modules` já tem na mão

O `cargo-modules` já constrói um grafo interno com nós e arestas, e já
extrai `path`, `name`, `kind`, `visibility`, `id`. Investigar, lendo o código
do fork e a estrutura que ele monta:

- Que informação sobre cada item o `cargo-modules` **já acessa** durante a
  construção do grafo, mas **não serializa** no JSON? (Ex.: o trait de um
  impl pode já estar acessível no ponto onde o nó é criado, só não é emitido.)
- Quão fácil seria emitir cada uma — é um campo já carregado em memória, ou
  exigiria nova consulta ao rust-analyzer?

Esta parte é a mais importante: campos que o fork **já tem** e só não emite
são baratos (como foi o `id`, que era o índice do petgraph já existente).

### Parte 2 — O que o rust-analyzer expõe (mas o cargo-modules ainda não usa)

Investigar a API do rust-analyzer (`ra_ap_*` crates que o `cargo-modules`
depende) para o que está disponível por item, mas exigiria o fork consultar
ativamente:

- Trait implementado (para métodos de impl).
- Generics e bounds do item.
- Assinatura (tipos de parâmetros e retorno, para funções).
- Associated types (que apareceram como caso de colisão na medição:
  `HtmlElem::Type`).
- Lifetimes.
- Atributos (`#[derive(...)]`, `#[cfg(...)]`, etc.).
- Qualquer outra coisa que a API exponha e que pareça relevante.

Para cada um: o rust-analyzer expõe isso de forma acessível ao fork, ou está
atrás de API privada / instável / cara de consultar?

### Parte 3 — Tabela de candidatos (o resultado central)

Uma tabela, um campo candidato por linha:

| Campo candidato | O que é | Disponibilidade (já tem / consulta fácil / consulta cara / não exposto) | Serve à pergunta da lente? | Caso de colisão que ele resolveria |
|-----------------|---------|-------------------------------------------------------------------------|----------------------------|-------------------------------------|

A última coluna conecta com a medição: vários padrões de colisão foram
identificados (`Display+Debug`, `From<X>+From<Y>`, `Add<Self>+Add<f64>`,
associated types como `HtmlElem::Type`, macros). Para cada campo candidato,
qual desses padrões ele ajudaria a distinguir/nomear?

---

## Restrições

- **Não modificar o fork.** Investigação de leitura — o que está disponível,
  a que custo. A escolha e a implementação são passos posteriores.
- **Não modificar nenhum crate do projeto-lente.**
- **Não emitir recomendação de qual descritor adotar.** A investigação
  levanta os candidatos com disponibilidade, custo e utilidade; a **escolha**
  é do autor. (Diferente das investigações anteriores, onde recomendar era
  ok — aqui a escolha do descritor é decisão de produto do autor, não técnica.)
- **Tudo em `lab/`** (sugestão: `lab/investiga-descritor-fork/`) ou como notas
  de leitura do fork. Não toca os outros experimentos.

---

## Resultado esperado

Relatório em `lab/investiga-descritor-fork/relatorio.md` com:

1. **Parte 1**: o que o `cargo-modules` já tem e não emite (campos baratos).
2. **Parte 2**: o que o rust-analyzer expõe e a que custo (campos mais caros).
3. **Parte 3**: a tabela de candidatos, com disponibilidade, filtro da
   pergunta-âncora, e o caso de colisão que cada um resolveria.
4. **Sem recomendação de escolha** — mas com a informação organizada para o
   autor escolher. Pode incluir agrupamentos ("estes 3 são baratos e servem;
   estes 2 são caros e de utilidade duvidosa") para facilitar a leitura, sem
   decidir.

---

## Por que esta investigação, e não escrever o descritor direto

O autor decidiu a direção (reforma estrutural, descritor rico) mas "rico"
não é especificável sem saber o que está disponível e a que custo. Escrever o
descritor sem essa investigação seria ou chutar um subconjunto (decisão
tomada por quem escreve o prompt, não pelo autor) ou pedir "tudo"
(over-engineering). Esta investigação transforma "rico" em uma lista de
candidatos avaliados, sobre a qual o autor faz uma escolha informada.

Depois desta investigação: o autor escolhe os campos, e aí sim um prompt de
modificação do fork emite exatamente o descritor escolhido — com escopo
definido, não aberto.
