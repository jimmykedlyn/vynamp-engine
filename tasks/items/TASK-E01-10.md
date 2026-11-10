# TASK-E01-10 — Endurecer engine e documentação dos limites

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E01-10` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E01-10` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E01-06`, `TASK-E01-09`, `TASK-E04-06` |
| Bloqueia | `TASK-E10-08`, `TASK-E15-06` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–15 dias úteis |
| Risco | alto |

## Entrega

Soak/recovery de engine e relatório de suporte por perfil.

## Boundary

- **Não** executa a matriz de soak por hardware e sistema. Desconectar DAC, trocar taxa, busy/sleep e stress de CPU em cada ambiente são `TASK-E10-08`; aqui o soak é uma condição controlada do engine, com a máquina de referência e um único perfil, para separar underrun do app de underrun do ambiente.
- **Não** instrumenta as métricas que o soak consome. Histograma de deadline, contadores RT por atomic e coleta fora da callback são `TASK-E10-07`; esta TASK usa esses números e publica o resultado, e criar histograma paralelo faria duas fontes para o mesmo gate.
- **Não** entrega a saída exclusiva nem os device profiles. WASAPI event-exclusive, IDs estáveis, capabilities e o controle de unplug/sleep pertencem a `TASK-E04-05`; sem esse backend, o soak daqui roda em `NullHost`/`CaptureHost` e isso é limitação declarada, não aprovação de taxa de relógio.
- **Não** redefine a semântica de cancelamento nem a state machine. Token idempotente, incremento de geração e o caminho `Stopping → Idle` são `TASK-E01-03`; aqui se prova que um decoder travado consegue ser cancelado e que o worker é juntado, não se desenha o token nem a tabela de estados.
- **Não** certifica modo transparente nem captura PCM24/32 exata. As condições ativas, os rótulos de fonte/saída e as combinações validadas são `TASK-E04-06`; o relatório desta TASK só transcreve o que aquela TASK mediu, e atribuir selo transparente a partir do soak seria declarar captura que ninguém mediu.
- **Não** decide channel mapping nem decodifica DSD. Mapas mono/stereo/5.1/7.1 com headroom no downmix e o decimador DSF/DFF são `TASK-E01-09`; os perfis multicanal do relatório consomem aqueles resultados e não os redefinem.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e01-10-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E01-10-AC-1` | 8 h de reprodução contínua de FLAC estéreo 24 bits a 192 kHz (gerado por `ffmpeg -f lavfi -i sine=frequency=997:sample_rate=192000:duration=28800 -ac 2 -c:a pcm_s24le` mais `-c:a flac`, 5 529 600 000 frames e cerca de 33 GiB, arquivo guardado fora do git com SHA-256 no MANIFEST como manda `quality/01-estrategia-testes.md`), na máquina de referência sem outro consumidor de áudio, com o ring SPSC e o dreno de `TASK-E01-05` e o gapless de `TASK-E01-06` no caminho | o soak roda as 8 h sem intervenção e o relatório classifica cada underrun publicado pela callback em causado pelo app (ring vazio por atraso do produtor) ou externo (stall do SO ou de outro processo na janela correspondente) | 8h24/192 controladas sem underrun causado pelo app |
| `TASK-E01-10-AC-2` | um decoder de teste no próprio `vynamp-engine` que bloqueia sem prazo no primeiro `next` de um frame, esperando num `Condvar` que nenhum teste sinaliza, com uma faixa de 1 h aberta, o engine em `Playing` e a geração de reprodução corrente | o Stop é comandado e, numa segunda passada, a troca de faixa incrementa a geração; em cada passada o teste mede o tempo até `Idle`, se o worker de decode é juntado e se o processo fecha dentro do shutdown limitado da spec | decoder hang pode ser cancelado |
| `TASK-E01-10-AC-3` | o relatório de suporte por perfil montado a partir das medições de AC-1 e AC-2 mais os resultados de `TASK-E01-09` (mapas 5.1/7.1 e DSD para PCM) e de `TASK-E04-06` (modo transparente e combinações validadas), com uma linha por combinação de taxa, canais, profundidade, backend e host e o nível de verificação de cada célula | o relatório é publicado no repositório e sua coluna de perfis é conferida contra a matriz gerada por `cargo run -p vynamp-output --example support_matrix` e contra `docs/support-matrix.md`, que um teste de `libs/output` mantém igual à saída do gerador | limitações publicadas. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && VYNAMP_SOAK_MEDIA=<flac-24-192-8h.flac> cargo run --release --example soak -- --duration 8h --json evidence/soak/flac-24-192-8h.json` sobre a fixture de 8 h a 192 kHz (5 529 600 000 frames, ~33 GiB, fora do git por hash, e no teste automatizado atrás de `#[ignore]` com a variável documentada); mais `cargo test --test recovery` com o decoder que bloqueia, e `cargo run -p vynamp-output --example support_matrix` para a matriz de perfis que o relatório transcreve | Esperado: no JSON do soak, `frames_presented` 5 529 600 000, `presented_regressions` 0, `underruns_app` 0 e todo `underruns_external` com a janela e a causa medidas; no recovery, o Stop durante o hang leva o engine a `Idle` dentro do shutdown limite da spec, o worker de decode é juntado, 0 threads sobram e o processo encerra; o relatório publica uma linha por perfil com nível de verificação e cada limitação nomeia a TASK dona. Hoje `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, só `pub const VERSION`), `Cargo.toml` sem dependências e nenhum `tests/` nem `examples/`, então `--example soak` e `--test recovery` falham por alvo inexistente; `libs/output` tem 42 `.rs` com `NullHost` e `CaptureHost` e nenhum backend WASAPI, então o soak mede a política do engine e não o relógio do dispositivo; e `libs/engine/specs/engine.md` §5, §6 e §10 ainda estão PENDENTES, sem o número do shutdown limite. |
| Hardware | necessário para AC-1 de verdade: um DAC USB que aceite 24 bits/192 kHz, e ele não está disponível — `evidence/PR/task-e01-02/environment.txt` registra apenas um endpoint WASAPI shared (`Headset (USB 384KZ HIFI Audio)`, 384 000 Hz, 2 canais, f32), que não é DAC 24/192 exclusivo, e este ambiente Windows não tem placa pro, aparelho Android, iOS nem macOS; a ausência é limitação registrada e o soak roda em `NullHost`/`CaptureHost`, provando a política de underrun e o cancelamento, não a taxa do relógio | ausência é limitação registrada, não aprovação |

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
