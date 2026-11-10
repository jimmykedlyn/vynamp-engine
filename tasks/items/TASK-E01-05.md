# TASK-E01-05 — Implementar mixer, drain e transições básicas

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E01-05` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E01-05` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M1 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E01-04` |
| Bloqueia | `TASK-E01-06`, `TASK-E04-01`, `TASK-E06-05`, `TASK-E10-07`, `TASK-E13-03` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 3–8 dias úteis |
| Risco | alto |

## Entrega

Mixer PCM, EOF drain, ramp play/stop e relógio de saída.

## Boundary

- **Não** implementa o grafo DSP nem crossfade de efeitos. O mixer aqui soma streams
  PCM e faz ramp de play/stop; `Compiled graph`, interpolação de parâmetro e crossfade
  equal-power entre cadeias são `TASK-E04-03`.
- **Não** faz gapless sample-accurate. O dreno de EOF garante que o último frame de
  conteúdo chega ao dispositivo; predecode da próxima faixa e trim de encoder
  delay/padding são `TASK-E01-06`. Drenar sem corte não é emendar duas faixas.
- **Não** conserta o viés do relógio do spike de `TASK-E01-02` por redesenho de contrato.
  A semântica `presented` já está na spec; esta TASK entrega o dreno que faz
  `presented` alcançar `content_frames` no fim, em vez de estabilizar em
  `content_frames − latency_frames`.
- **Não** aplica EQ, ganho, pan ou ReplayGain. O mixer soma no nível que recebe; a cadeia
  de processamento por stream é `TASK-E04-01`, que depende desta TASK justamente porque
  precisa de um grafo estável para pendurar os nós.
- **Não** escolhe backend, negocia formato nem reabre dispositivo em troca de sample
  rate. Amostragem, exclusividade e perfis de dispositivo são da `vynamp-output`
  (`TASK-E04-05`); se a troca de taxa quebra o período negociado, isso é limitação
  reportada, não resolvida aqui.
- **Não** arredonda nem dither. O contador de frames é exato em `u64` e o último frame de
  conteúdo é entregue como está; conversão de bit depth e dither são da `TASK-E04-02`.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e01-05-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E01-05-AC-1` | uma faixa cujo frame count não é múltiplo do período negociado do dispositivo | a reprodução chega ao EOF e o engine drena até o último frame de conteúdo | Fim não corta frames |
| `TASK-E01-05-AC-2` | a reprodução pausada com samples ainda no ring e buffers de decode em voo | o usuário dá Stop, abre outra faixa e dá Play | pause/stop não deixam áudio antigo em próximo play |
| `TASK-E01-05-AC-3` | duas fontes iguais por construção (o mesmo arquivo copiado duas vezes) e o volume default em 100% | o A/B alterna entre elas por 60 s e compara posição em ticks, frames apresentados e clock do dispositivo | volumes default não amplificam. A/B usa ticks/frames válidos sem drift e encerra ao trocar fonte. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine`, com a fixture de faixa cujo frame count não é múltiplo do período negociado (AC-1), o roteiro pause→stop→outra faixa→play (AC-2) e o A/B de 60 s entre duas cópias do mesmo arquivo (AC-3) | verde no CI Gate, e a contagem de frames drenados igual ao frames restantes depois do EOF, sem truncar o último período |
| Oráculo independente | os dois JSON de `evidence/PR/task-e01-02/` (`probe-steady.json` e `probe-stall.json`), comparando a posição apresentada final com o conteúdo produzido: hoje 11 507 474 e 11 507 560 contra 11 520 000 de conteúdo; mais `sox --i` ou `ffprobe -show_streams` sobre o arquivo de A/B para fixar o frame count de referência | as duas posições finais sobem para 11 520 000 e o A/B termina sem drift — valores lidos dos JSON e do relatório, não "parece certo"; enquanto o dreno não existir, os dois números continuam abaixo do conteúdo |
| Hardware | necessário para AC-3 e para a posição real: a medição de referência foi em WASAPI shared, `Headset (USB 384KZ HIFI Audio)`, 384 000 Hz, 2 canais, buffer 3 840 frames; aqui não há dispositivo algum | ausência é limitação registrada, não aprovação: sem dispositivo o teste roda em backend de teste e prova dreno e ramp, não a taxa do relógio |

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
