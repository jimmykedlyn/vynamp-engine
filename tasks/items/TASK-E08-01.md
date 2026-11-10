# TASK-E08-01 — Implementar jobs persistidos

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E08-01` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E08-01` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M2 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E03-01` |
| Bloqueia | `TASK-E03-08`, `TASK-E08-02`, `TASK-E08-03`, `TASK-E08-04`, `TASK-E08-05`, `TASK-E18-01`, `TASK-E18-02` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–12 dias úteis |
| Risco | médio |

## Entrega

Scheduler de jobs com checkpoints, leases, progresso e pools limitados.

## Boundary

- **Não** implementa o schema SQLite nem as migrations que guardam o estado da fila: isso é `TASK-E03-01`, que entrega WAL, write actor e backup. Aqui o scheduler apenas consome essa API; criar tabela própria faria dois donos do mesmo schema.
- **Não** implementa a conversão, o registry de encoders ou a validação pós-escrita do arquivo convertido: isso é `TASK-E08-02` e, na ferramenta, `TASK-E18-02`. A confusão é plausível porque o primeiro consumidor da fila é o conversor e o teste de cancelamento passa por um encoder.
- **Não** calcula loudness, ReplayGain nem BPM, nem define o padrão ou a versão da análise: isso é `TASK-E08-03`. Aqui só se agenda e se contabiliza o progresso desse job, não o resultado numérico dele.
- **Não** define templates de renomeação, whitelist de campos, sanitização de nomes reservados, detecção de colisão nem dedupe: isso é `TASK-E08-04`. O scheduler executa o plano que ela produz e não decide nome de arquivo.
- **Não** implementa sleep timer, alarmes, regras de DST nem quiet hours: isso é `TASK-E08-05`. A palavra scheduler aparece nas duas TASKs, mas lá é agenda de reprodução por relógio de parede e aqui é fila de trabalho com lease.
- **Não** mexe nos cancellation tokens do grafo de áudio nem na state machine de playback: isso é `TASK-E01-03`. O cancelamento desta TASK é por job e por fila, sobre arquivos e processos, e misturar os dois faria um Stop de reprodução derrubar conversão em andamento.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e08-01-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E08-01-AC-1` | uma fila de 40 jobs de cópia de fixture descrita em `tests/fixtures/jobs/queue-40.json`, checkpoint gravado após cada job concluído, lease de 30 s e o processo encerrado com TerminateProcess (o equivalente a crash, sem Drop nem flush) logo depois do checkpoint do job 12 | o scheduler é reaberto no mesmo volume a partir do mesmo arquivo de estado, sem intervenção manual e sem comando explícito de retomada | Crash retoma jobs seguros |
| `TASK-E08-01-AC-2` | um job de escrita que já produziu o temporário `destino.flac.vynamp-tmp-8f3a` no mesmo volume do destino, com a fonte `fonte.flac` aberta somente para leitura e o SHA-256 dela registrado antes do job começar | o cancelamento do job é solicitado no meio da escrita do temporário, e o job é observado até o seu estado terminal | cancelar remove temporários sem apagar source |
| `TASK-E08-01-AC-3` | uma fila de 30 jobs na qual 4 falham de formas distintas: 1 arquivo com lock exclusivo tomado por outro processo, 1 extensão sem decoder registrado, 1 destino em volume cheio e 1 fonte truncada; os outros 26 concluem com sucesso | a fila é drenada até o fim e o progresso e a lista de erros são lidos pela API pública do scheduler | erros parciais são visíveis e progresso é consistente. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --test jobs` sobre a fixture `tests/fixtures/jobs/` (fila de 40 jobs, fontes com SHA-256 registrado, diretório de temporários próprio) e o binário `examples/jobs_crash.rs`, que mata o processo em três pontos: antes do checkpoint, entre checkpoint e commit, e depois do rename; mais `cargo test --test jobs -- --ignored` com `VYNAMP_JOBS_CRASH_POINTS` para a varredura completa dos pontos de kill | Esperado: após cada um dos três pontos de kill, exatamente 40 jobs concluídos e 0 duplicados, 0 temporários órfãos em `tests/fixtures/jobs/tmp/`, cada fonte com SHA-256 igual ao registrado e cada destino relido com taxa, canais e duração iguais aos da fonte; no cancelamento, o temporário ausente do diretório, a fonte com SHA-256 inalterado e o job no estado `Cancelled`; na fila de 30, 26 concluídos, 4 falhados com código distinto por causa, e a soma concluídos + falhados + cancelados igual a 30 em todo instante amostrado. Hoje nada disso existe: `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, 4 linhas, só `pub const VERSION`), `Cargo.toml` sem dependências e nenhum diretório `tests/` ou `examples/`, então `cargo test --test jobs` falha por alvo inexistente e não prova nada. |
| Hardware | nenhum: o corpus é estado em disco mais kill e restart de processo local; nenhum teste abre dispositivo de áudio, placa pro, aparelho Android, iOS ou macOS, e este ambiente Windows 11 x64 basta, incluindo o volume cheio simulado por quota de fixture | ausência é limitação registrada, não aprovação |

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
