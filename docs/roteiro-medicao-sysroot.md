# Roteiro de Medição — `export-json --sysroot` contra um crate real

> **Objetivo da medição**: decidir *onde* o filtro de stdlib deve morar
> (no fork, ou na lente como L1). Essa decisão depende de dois números que só
> aparecem com dado real: quanto a stdlib infla a saída, e se os derives do seu
> código passaram a aparecer com sysroot ligado.
>
> **Não é** uma decisão de arquitetura nova — é a verificação que a alimenta.
> Roda na sua máquina, onde o binário do fork e a toolchain Rust existem.

---

## 0. Pré-requisito

Os comandos de análise usam `jq`. Verifique:

```bash
jq --version
```

Se não tiver: `sudo apt install jq` (Debian/Ubuntu), `brew install jq` (macOS),
ou use a alternativa em Python na seção 5 (não precisa instalar nada além do
Python que você já tem).

---

## 1. Escolher o crate-alvo

Escolha **um crate real seu** — não um fixture de teste. De preferência um de
tamanho médio que represente o tipo de código que você escreve no dia a dia
(com structs, enums, traits, e derives — porque é o derive que estamos
medindo). Evite por ora um workspace gigante; um crate único de algumas
dezenas a poucas centenas de itens é o ideal para a primeira leitura.

---

## 2. Gerar as duas saídas (sem e com sysroot)

O ponto da medição é **comparar**. Gere os dois arquivos:

```bash
cd /caminho/do/seu/crate

# Sem sysroot (o que tínhamos antes)
cargo modules export-json --compact > /tmp/lente_sem_sysroot.json

# Com sysroot (a correção de fidelidade)
cargo modules export-json --sysroot --compact > /tmp/lente_com_sysroot.json
```

Use `--compact` para os arquivos ficarem menores e o `jq` processar rápido.
Se algum comando falhar, anote o erro — pode ser toolchain (o fork exige
rust 1.91+, edition 2024) e isso é informação útil.

---

## 3. Os números que decidem (com `jq`)

Rode este bloco para **cada** arquivo, trocando o nome:

```bash
ARQ=/tmp/lente_com_sysroot.json   # depois repita com /tmp/lente_sem_sysroot.json

echo "=== $ARQ ==="
echo "tamanho do arquivo:"; ls -lh "$ARQ" | awk '{print $5}'
echo "nós totais:";   jq '.nodes | length' "$ARQ"
echo "arestas totais:"; jq '.edges | length' "$ARQ"

# Nós de stdlib (o "ruído" candidato a filtro)
echo "nós de stdlib:"; \
  jq '[.nodes[] | select(.path | test("^(std|core|alloc|proc_macro|test)(::|$)"))] | length' "$ARQ"

# Nós do seu crate (o que interessa)
echo "nós NÃO-stdlib:"; \
  jq '[.nodes[] | select(.path | test("^(std|core|alloc|proc_macro|test)(::|$)") | not)] | length' "$ARQ"

# Arestas que apontam para a stdlib (dependências via derive/uso de stdlib)
echo "arestas apontando p/ stdlib:"; \
  jq '[.edges[] | select(.to | test("^(std|core|alloc|proc_macro|test)(::|$)"))] | length' "$ARQ"

# Distribuição de tipos de item
echo "tipos (kind):"; jq -r '[.nodes[].kind] | group_by(.) | map({(.[0]): length}) | add' "$ARQ"
```

---

## 4. O que observar (a leitura dos números)

Anote, dos dois arquivos lado a lado:

| Pergunta | Como responder |
|----------|----------------|
| **Quanto a stdlib infla?** | `nós de stdlib` ÷ `nós totais` no arquivo *com sysroot*. Se for uma fração pequena (digamos < 20%), o ruído é administrável. Se dominar (> 50%), é pesado. |
| **Os derives apareceram?** | Compare `nós totais` e `arestas totais` entre sem-sysroot e com-sysroot. O aumento é o que o sysroot revelou (os `impl` de derives e o que eles puxam). Se os números mal mudaram, ou seu código usa poucos derives, ou algo não expandiu. |
| **O JSON ficou grande demais?** | `tamanho do arquivo` com sysroot. Se forem alguns KB ou poucos MB, a lente carrega tranquilo (filtrar na lente é viável). Se forem dezenas de MB, carregar tudo para filtrar depois é desperdício (filtrar no fork ganha). |
| **Quanto do raio vinha de stdlib?** | `arestas apontando p/ stdlib` ÷ `arestas totais`. Mede quanto do "raio de impacto" seria poluído por dependências de stdlib se não filtrássemos. |

---

## 5. Alternativa sem `jq` (Python puro)

Se preferir não instalar `jq`, salve como `medir.py` e rode
`python3 medir.py /tmp/lente_com_sysroot.json`:

```python
import json, sys
from collections import Counter

d = json.load(open(sys.argv[1]))
nodes, edges = d["nodes"], d["edges"]
STD = ("std", "core", "alloc", "proc_macro", "test")
is_std = lambda p: p.split("::")[0] in STD

n_std = [n for n in nodes if is_std(n["path"])]
e_to_std = [e for e in edges if is_std(e["to"])]

print("arquivo:", sys.argv[1])
print("nós totais:", len(nodes))
print("arestas totais:", len(edges))
print("nós de stdlib:", len(n_std), f"({100*len(n_std)//max(len(nodes),1)}%)")
print("nós NÃO-stdlib:", len(nodes) - len(n_std))
print("arestas p/ stdlib:", len(e_to_std), f"({100*len(e_to_std)//max(len(edges),1)}%)")
print("tipos:", dict(Counter(n["kind"] for n in nodes)))
print("visibilidades:", dict(Counter(n["visibility"] for n in nodes)))
print("relations:", dict(Counter(e["relation"] for e in edges)))
```

Rode para os dois arquivos (sem e com sysroot) e compare.

---

## 6. O que trazer de volta

Para decidirmos onde o filtro mora, traga:

1. Os números do passo 3/5 para os **dois** arquivos (sem e com sysroot).
2. O nome e o tamanho aproximado do crate (quantos arquivos/itens, se souber).
3. Qualquer erro que apareceu ao rodar (especialmente de toolchain/sysroot).

Com isso, a decisão "filtrar no fork vs. filtrar na lente (L1)" sai dos números,
não do argumento:

- **stdlib é fração pequena e JSON administrável** → filtrar na lente (L1).
  Mantém a fonte fiel (emite tudo), põe a decisão de esconder no lattice da
  lente, e deixa reversível (ligar/desligar o filtro sem tocar no fork).
- **stdlib domina e JSON enorme** → filtrar no fork, reusando o `Filter` que o
  `dependencies` já tem. Economiza dados antes de a lente carregar.

---

## Nota sobre a fidelidade

Ligar `--sysroot` é a correção de fidelidade que decidimos: sem ele, o grafo do
rust-analyzer não expande os derives, então dependências reais (ex.: via
`#[derive(Clone)]`) ficam invisíveis. Com ele, elas existem no grafo e o printer
as emite. O filtro de stdlib é uma questão **separada e posterior** — é sobre
clareza (tirar ruído), não sobre fidelidade (ver o que existe). Primeiro
garantimos que a informação existe (sysroot); depois decidimos onde escondê-la
da tela (filtro). Não inverta a ordem: filtrar antes de ligar sysroot
esconderia a informação antes mesmo de ela aparecer.
