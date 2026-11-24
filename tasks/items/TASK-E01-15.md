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
| Critérios da spec | `specs/engine.md` §5 (`ENGINE-AC-16`, `ENGINE-AC-17`) e `specs/pipeline.md` §5 (`PIPE-AC-12`) — rascunho não aprovado |
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
| `TASK-E01-15-AC-2` | reprodução em curso, com a rampa de 480 frames de `specs/pipeline.md` §6 | o usuário dá play ou stop | play e stop aplicam ramp sem clique audível. |

## Prova

"Sem clique audível" é provado sobre as amostras, não por audição (`specs/pipeline.md`
§6, `PIPE-REQ-12`, `PIPE-AC-12`, `play_stop_ramp_has_no_step`): play e stop aplicam
rampa linear de ganho de 480 frames da taxa de saída (10 ms a 48 kHz). Com amostras
normalizadas em [−1, 1] e uma senoide de amplitude 1,0 a 1 kHz e 48 kHz, nenhum degrau
entre amostras consecutivas do mesmo canal excede o maior degrau da senoide somado ao
passo da rampa, 1/480.

"Volumes default não amplificam" é provado por igualdade amostra a amostra entre entrada
e saída do mixer com volume padrão (`ENGINE-AC-16`, `default_volume_is_bit_exact`) e
pelo teto de volume: pedido de 2.0 é aplicado como 1.0 e a saída segue idêntica à
entrada (`ENGINE-AC-17`, `volume_above_unity_is_clamped`).

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine`, com fonte de volume padrão comparada amostra a amostra (AC-1, `ENGINE-AC-16` e `ENGINE-AC-17`) e play/stop sobre a senoide de `PIPE-AC-12`, medindo o degrau máximo dentro da rampa (AC-2) | verde no CI Gate; nenhuma amostra de saída difere da entrada com volume padrão; o maior degrau medido na rampa fica dentro do limite da seção Prova |
| Hardware | não é necessário para os critérios: o ramp é provado sobre as amostras em backend de teste; aqui não há dispositivo algum | ausência é limitação registrada, não aprovação: sem dispositivo o teste prova a forma da rampa e o ganho unitário, não a audição em dispositivo real |

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
