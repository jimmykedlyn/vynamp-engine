# TASK-E04-04 — Entregar EQ paramétrico e presets

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E04-04` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E04-04` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E04-03` |
| Bloqueia | `TASK-E02-07`, `TASK-E04-07`, `TASK-E04-09` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–15 dias úteis |
| Risco | alto |

## Entrega

32bands/tipos/Q/canal, EQ20/31 e preset schema/precedence.

## Boundary

- **Não** entrega o EQ-10, o preamp por modo, o volume/pan nem o ReplayGain. O gráfico de dez bandas (`31,25 Hz … 16 kHz`, `Q ≈ 1,41`) e o preamp são `TASK-E04-01`; reimplementar o mesmo biquad aqui faria dois donos do modo M1 e duas curvas para o mesmo botão.
- **Não** implementa o EQ-64 gráfico nem o PEQ de 64 filtros. Os dois são M5 e `TASK-E12-10` é a dona; o teto desta TASK é o de 32 filtros, que é o `maxItems: 32` que `schemas/audio-preset.schema.json` já declara hoje. A grade de 64 bandas aparece no AC-2 só como fixture de disponibilidade por Nyquist, lida da mesma tabela de filtros: ela não vira modo selecionável, não ganha banda editável e não entra no estado serializado.
- **Não** constrói o editor de curva, os sliders por banda, o seletor de cinco modos nem o contador n/32 na tela. `TASK-E02-07` é a dona da integração na UI; aqui se prova a resposta numérica e o estado, e um controle manipulável não é evidência desta TASK.
- **Não** resolve a precedência sessão > faixa > gênero > dispositivo > global nem importa AutoEQ com origem e licença. Perfis por dispositivo e headphone preset são `TASK-E04-09`; aqui o preset é um arquivo versionado e um estado em memória, sem escopo por dispositivo.
- **Não** faz o crossfade da troca de modo nem a interpolação de 10–50 ms do ganho. `TASK-E04-03` entrega o grafo compilado, o smoothing e o swap por geração; esta TASK entrega coeficientes estáveis e o estado, não a transição entre dois estados.
- **Não** é a proteção final true-peak. Um preset com várias bandas em +15 dB satura a saída, e o guard fixo depois do master e antes da quantização é `TASK-E17-07`; aqui o filtro instável é recusado antes do swap, não limitado depois.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e04-04-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E04-04-AC-1` | um PEQ de 32 filtros — 8 peaking, 4 low shelf, 4 high shelf, 4 low pass, 4 high pass, 4 notch e 4 band pass — com frequências de 20 Hz a 18 kHz, Q entre 0,7 e 8 e ganho de +6 dB e −6 dB nas bandas que usam ganho, aplicado no tap pós-EQ e avaliado nas taxas de 44 100, 48 000, 96 000 e 192 000 Hz | a resposta em frequência de cada estado é medida por impulso e por sweep e comparada, ponto a ponto de 10 Hz a min(40 kHz, Fs/2), com a cascata calculada fora do código sob teste a partir das mesmas fórmulas RBJ | Curva±0.2dB da referência |
| `TASK-E04-04-AC-2` | o EQ-31 com a banda de 20 kHz (19 952,6 Hz) em +6 dB e o EQ-64 com as 64 bandas habilitadas, ambos rodando a 44 100 Hz, e o EQ-31 com a banda de 16 kHz na mesma condição | a taxa é trocada para 32 000 Hz, depois para 48 000 Hz e de volta a 44 100 Hz, e em cada estado se lê a disponibilidade banda por banda, o valor guardado, o motivo exibido e a curva calculada | frequências válidas porNyquist |
| `TASK-E04-04-AC-3` | um preset `peq` válido já carregado, com 32 filtros e preamp de −3 dB, serializado byte a byte como referência, e quatro arquivos de import ao lado: um com `q` 0,05, um com `gain_db` 40, um com `frequency_hz` 45 000 e um com o campo `q` como texto `NaN` | cada arquivo é importado em sequência sobre o preset carregado e, após cada recusa, são lidos o estado ativo, a serialização do preset, o código de erro e o JSON pointer da mensagem | import inválido mantém preset anterior. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --test eq_parametric --locked`, com o preset de 32 filtros em `tests/fixtures/eq/peq32.json` e os quatro imports inválidos em `tests/fixtures/eq/invalid/`, mais a referência em `tools/eq_ref.py` — Python puro com `cmath`, sem numpy nem scipy, que monta os coeficientes RBJ por tipo e avalia `H(f) = G_preamp · ∏ H_k(e^{j2πf/Fs})` nos mesmos 512 pontos logarítmicos de 10 Hz a `min(40 kHz, Fs/2)` que o nó reporta | Esperado: diferença ≤ 0,2 dB da referência em cada um dos 512 pontos nas quatro taxas para as 16 bandas que usam ganho, alternando +6 e −6 dB (as outras 16 de low pass, high pass, notch e band pass entram pelo Q), e ≤ 0,1 dB entre a curva que o nó exibe e a cascata calculada; a 44 100 Hz a banda de 20 kHz do EQ-31 (19 952,6 Hz, acima do limite 0,45 × 44 100 = 19 845 Hz) e a banda 64 do EQ-64 (20 000 Hz) ficam indisponíveis com o motivo escrito e o valor guardado intacto, enquanto a banda de 16 kHz do EQ-31 (15 848,9 Hz) e as bandas 1 a 63 do EQ-64 seguem disponíveis, a 32 000 Hz caem as bandas 16 kHz e 20 kHz do EQ-31 e as bandas 62 a 64 do EQ-64 (a 61, de 14 390 Hz, fica no limite 14 400), a 48 000 Hz nenhuma cai com o limite de 21 600 Hz, e em nenhuma taxa o valor é movido em silêncio; no import inválido o preset anterior continua ativo, a serialização continua idêntica byte a byte à referência e cada um dos quatro imports é recusado com o JSON pointer certo (`/bands/3/q` para o Q 0,05, `/bands/7/gain_db` para o ganho 40, `/bands/11/frequency_hz` para os 45 000 Hz e `/bands/15/q` para o Q com texto `NaN`), sem nenhum deles deixar valor parcial no estado. Hoje nada disso existe: `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, 6 linhas, só `pub const VERSION`), `cargo test --locked` roda 0 testes em 2 suítes, e `schemas/audio-preset.schema.json` ainda é `schema_version` 1, sem `mode`, sem `enabled` por filtro e sem `slope_db_oct` — o v2 com migração v1→v2 é entrega desta TASK e não existe em lugar nenhum. |
| Hardware | nenhum: a resposta é medida em amostras de PCM geradas fora de linha, sem nenhum dispositivo de áudio aberto; o que falta não é hardware e sim a implementação e o preset v2, porque `libs/engine` tem 1 arquivo `.rs` de 6 linhas e `specs/equalizadores.md` §5 ainda está vazia, sem nenhum Given/When/Then autorado. | ausência é limitação registrada, não aprovação |

## DoD — pronto para review

- [ ] Todos os critérios de aceite com evidência
- [ ] `cargo fmt --check` e `cargo clippy --all-targets -- -D warnings` limpos
- [ ] `cargo test` verde; testes ignorados listados com motivo
- [ ] Spec do módulo atualizada se a implementação revelou algo novo
- [ ] `backlog.json` com o estado correto
- [ ] Revisão feita por contexto limpo, não pelo implementador
- [ ] PR aberto com template preenchido; CI Gate verde no SHA final
## Notas de execução

Preenchido durante a execução, não antes.
