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
| Critérios da spec | `specs/engine.md` §5 (`ENGINE-AC-08b`) e `specs/pipeline.md` §5 (`PIPE-AC-01`) — rascunho não aprovado |
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
| `TASK-E01-15-AC-2` | reprodução em curso | o usuário dá play ou stop | play e stop aplicam ramp sem clique audível. |

## Prova

"Sem clique audível" é provado sobre as amostras, não por audição: o ramp de play e o de
stop são rampas lineares de ganho com duração declarada em frames, e em nenhum ponto da
rampa a diferença entre duas amostras consecutivas do mesmo canal é maior que o maior
degrau entre amostras consecutivas da fonte somado ao passo da rampa (`1 / duração`).
"Volumes default não amplificam" é provado por igualdade bit a bit entre entrada e saída
do mixer com volume 1.0 (`ENGINE-AC-08b`, `default_volume_is_bit_exact`).

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine`, com fonte de volume padrão comparada amostra a amostra (AC-1) e play/stop sobre fonte de amplitude conhecida, medindo o degrau máximo dentro da rampa (AC-2) | verde no CI Gate; nenhuma amostra de saída difere da entrada com volume padrão; o maior degrau medido na rampa fica dentro do limite da seção Prova |
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
