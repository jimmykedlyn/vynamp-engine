# TASK-E01-15 — Implementar mixer PCM e ramp de play/stop

> Autorado à mão até o próximo gerador; os campos de estado vêm de `tasks/backlog.json`
> do umbrella. A coluna `Then` é o requisito aprovado, copiado literal do backlog;
> `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E01-15` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E01-15` |
| Critérios da spec | `specs/engine.md` §5 (`ENGINE-AC-16`, `ENGINE-AC-17`, `ENGINE-AC-18`) e `specs/pipeline.md` §5 (`PIPE-AC-12`) — rascunho não aprovado |
| Fase | M1 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E01-05` |
| Bloqueia | `TASK-E04-01` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 2–5 dias úteis |
| Risco | médio |

## Entrega

Mixer PCM que soma streams e ramp de play/stop sem clique.

## Boundary

- **Não** faz o dreno de EOF nem o relógio de saída (`presented`): são da `TASK-E01-05`,
  de que esta TASK depende. O mixer entrega ao caminho de saída que ela já drena.
- **Não** faz o grafo DSP nem o crossfade entre faixas. `Compiled graph`, interpolação de
  parâmetro e crossfade equal-power são da `TASK-E04-03`.
- **Não** aplica EQ, ganho de usuário ou ReplayGain. A cadeia de processamento por stream
  é da `TASK-E04-01`; aqui o volume padrão é 1.0 e o mixer não amplifica.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e01-15-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E01-15-AC-1` | uma fonte com volume padrão (100%) | a reprodução passa pelo mixer | Volumes default não amplificam. |
| `TASK-E01-15-AC-2` | cosseno de amplitude 1,0 a 1 kHz e 48 kHz, começando na fase 0 (x₀ = 1), amostras normalizadas em [−1, 1]; stop pedido no frame 4800 (um pico) | play no frame 0 e stop no frame 4800 | play e stop aplicam ramp sem clique audível. |

## Prova

"Sem clique audível" é provado sobre as amostras, não por audição (`specs/pipeline.md`
§3.1, `PIPE-REQ-12`, `PIPE-AC-12`, `play_stop_ramp_has_no_step`). A rampa dura 10 ms
convertidos em frames na taxa do estágio do mixer, arredondados para baixo: 480 frames a
48 kHz. Com o cosseno do AC-2, a primeira amostra de saída é 0; do frame 480 ao 4799 a
saída é igual à entrada; a partir do frame 4800 + 480 a saída é 0; nenhum degrau entre
amostras consecutivas do mesmo canal passa de 2·sin(π/48) + 1/480; e, no meio da rampa
linear de play, `y[240] == x[240] / 2`. Saída sempre zero
reprova a segunda condição, e rampa ausente reprova a primeira e a terceira.

Os critérios de volume e de soma valem a partir do frame N da rampa de play
(`specs/pipeline.md` §3.1). "Volumes default não amplificam" é provado por igualdade
amostra a amostra entre entrada e saída do mixer com volume padrão (`ENGINE-AC-16`,
`default_volume_is_bit_exact`) e pelo teto de volume de `specs/engine.md` §3: com o
volume em 0,5, `SetVolume(2.0)` devolve `InvalidArgument`, o volume continua 0,5 e a
saída é a entrada × 0,5 (`ENGINE-AC-17`, `volume_above_unity_is_rejected`). A soma do
mixer é provada com a stream A = 0,1·(k mod 4) e a stream B = 0,05 constante, com volume
padrão: cada amostra de saída é A[k] + B[k] (`ENGINE-AC-18`, `mixer_sums_streams`).

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine`, com fonte de volume padrão comparada amostra a amostra, `SetVolume(2.0)` recusado e soma de duas streams (AC-1, `ENGINE-AC-16` a `ENGINE-AC-18`) e play/stop sobre o cosseno de `PIPE-AC-12` (AC-2) | verde no CI Gate; nenhuma amostra de saída difere da entrada com volume padrão; `SetVolume(2.0)` devolve `InvalidArgument` e a saída segue a entrada × 0,5; a saída é A[k] + B[k] na soma, todos a partir do frame N da rampa de play; a saída da rampa cumpre as cinco condições da seção Prova |
| Hardware | não é necessário para os critérios: o ramp é provado sobre as amostras em backend de teste; aqui não há dispositivo algum | ausência é limitação registrada, não aprovação: sem dispositivo o teste prova a forma da rampa, o ganho unitário e a soma, não a audição em dispositivo real |

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
