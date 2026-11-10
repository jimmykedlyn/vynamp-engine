# TASK-E01-03 — Implementar state machine e generation cancellation

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E01-03` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E01-03` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M1 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E01-01`, `TASK-E01-02` |
| Bloqueia | `TASK-E01-04`, `TASK-E02-03`, `TASK-E03-02`, `TASK-E05-01`, `TASK-E08-05`, `TASK-E08-06`, `TASK-E08-07`, `TASK-E10-02`, `TASK-E13-02`, `TASK-E14-02`, `TASK-E16-02` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 3–8 dias úteis |
| Risco | alto |

## Entrega

Domain playback states, commands/events e cancellation tokens.

## Boundary

- **Não** produz nem mistura uma única amostra. Esta TASK entrega o domínio — estados,
  comandos, eventos e tokens de cancelamento; o ring, o mixer PCM e o dreno de EOF são
  `TASK-E01-05`. Uma state machine verde não prova que o áudio saiu.
- **Não** abre o dispositivo, não negocia formato e não publica o relógio. A latência e a
  posição apresentada são medidas pela callback de `vynamp-output`, fixadas no spike de
  `TASK-E01-02`; aqui só se consome o que ela informa. Sem `vynamp-output` no teste, o
  relógio é falso de propósito.
- **Não** implementa seek de verdade. Incrementar a geração e recusar buffer de geração
  velha é desta TASK; resolver tempo→byte/frame, priming e preskip é do `vynamp-decode`
  (`TASK-E01-04`, `TASK-E17-02`). Um seek que só incrementa geração não reposiciona
  decodificador nenhum.
- **Não** decide gapless nem trim de encoder delay/padding. `TASK-E01-06` é dona do
  predecode da próxima faixa e do corte validado; aqui só se garante que a troca de faixa
  invalida a geração anterior.
- **Não** desenha a recuperação de dispositivo (`Reconnecting`, `DeviceLost`, `Error`) nem
  o soak. O caminho coberto é `Idle → Opening → Buffering → Playing ↔ Paused → Seeking →
  Stopping → Idle`; recovery fica em `TASK-E01-10`, e erro de backend chega como evento
  que esta TASK só transporta.
- **Não** persiste estado nem expõe comandos à UI. Ligar commands/events aos painéis
  clássicos é `TASK-E02-03`; não há nada entre sessões para guardar.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e01-03-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E01-03-AC-1` | a state machine em Opening com o produtor de decode emitindo buffers e os workers de I/O vivos | um comando Stop chega antes do primeiro buffer entrar no ring e o teste espera todos os workers terminarem | Stop durante Opening cancela tudo |
| `TASK-E01-03-AC-2` | a reprodução em Playing com dois buffers da geração 1 já publicados no ring | um seek incrementa a geração para 2 e o consumidor drena o ring até o último frame | seek/troca descarta buffers antigos |
| `TASK-E01-03-AC-3` | o estado já em Idle depois de um Stop ter sido aplicado | o comando Stop é enviado três vezes seguidas e o contador de eventos do domínio é lido | repeated stop é idempotente. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine`, com as três suítes de AC-1..AC-3: Stop em `Opening`, seek que incrementa a geração no ring, e Stop repetido três vezes | verde no CI Gate, com os três testes nomeados no relatório — não um teste genérico de estado |
| Oráculo independente | contador do consumidor de teste, escrito no harness e não no domínio, somando os frames que ele de fato retirou do ring; a expectativa vem do número de buffers que a fixture declarou produzir por geração, não do que a state machine relata | depois do seek de AC-2 o consumidor soma exatamente os buffers da geração 2 e nenhum da geração 1; depois do 1º Stop de AC-3 o número de eventos aplicados é 1 e o 2º e o 3º não o movem — dois contadores contados fora do código sob teste, não "parece certo" |
| Hardware | nenhum nesta TASK: o domínio é determinístico e roda em processo, mas a corretude de `presented` depende do `vynamp-output`, cuja medição real foi feita na `TASK-E01-02` em WASAPI shared | ausência é limitação registrada, não aprovação: sem a callback real, o teste de geração prova descarte, não audibilidade |

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
