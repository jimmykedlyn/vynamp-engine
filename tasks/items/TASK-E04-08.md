# TASK-E04-08 — Entregar tempo, pitch e efeitos criativos

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E04-08` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E04-08` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E04-03`, `TASK-E04-07` |
| Bloqueia | `TASK-E04-10`, `TASK-E17-19` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–15 dias úteis |
| Risco | alto |

## Entrega

Tempo0.5–2x/pitch±12/speed, reverb/echo/chorus/flanger/silence trim.

## Boundary

- **Não** implementa o resampler nem a conversão para a taxa de saída. Tempo e pitch mudam a duração e a altura do stream, e a taxa de saída continua sendo uma só, a de `TASK-E04-02`; converter taxa aqui criaria um segundo resampler no caminho e mataria a conta de latência do final.
- **Não** entrega compressor, limiter, bass/treble, mono nem roteamento. `TASK-E04-07` é a dona de dynamics e tone e é dependência desta TASK; reverb e delay somam energia e quem contém esse pico é o limiter dela, não outro nó daqui.
- **Não** é a proteção final true-peak. Reverb com decay de 8 s e fontes correlacionadas em crossfade somam pico, e o guard de posição fixa depois do master é `TASK-E17-07`.
- **Não** drena os tails no stop, no seek ou na troca de faixa, nem faz o ramp de play/stop. O mixer, o dreno de EOF e o ramp são `TASK-E01-05`; aqui o nó só declara `tail_frames()`, reseta a geração e devolve o buffer sem NaN.
- **Não** constrói a UI de efeitos nem a preferência de velocidade por categoria. O painel de efeitos e da cadeia é `TASK-E02-07`; guardar a velocidade por categoria, inclusive a de podcast, e retomá-la por episódio é `TASK-E08-09`, cujo REQ-E08-09 diz literalmente "velocidade por categoria". `TASK-E05-05` só entrega feed e download de episódio, e sem essa separação a fila de podcasts e o nó de tempo disputariam o mesmo campo de velocidade.
- **Não** define o schema de preset nem a migração. `TASK-E04-04` é a dona do preset versionado; aqui cada parâmetro só declara unidade, faixa, default e se é suavizável no contrato de nó de `TASK-E04-03`.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e04-08-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E04-08-AC-1` | um episódio de podcast cuja preferência de velocidade de 1,5× está gravada no escopo de podcast, seguido na fila por `st16-44k1.flac` de `libs/decode/tests/fixtures/flac` (44 100 Hz, 2 canais, 4 410 frames no STREAMINFO, SHA-256 conferido no `MANIFEST.txt` de lá) com velocidade 1,0× | o episódio é tocado até o fim com a preferência aplicada e a taxa lida no snapshot do nó de tempo é registrada, depois a faixa de música é carregada no mesmo pipeline e, no primeiro e no último bloco, são lidos os frames entregues, a duração apresentada e o estado do nó de tempo | Música não herda velocidade podcast |
| `TASK-E04-08-AC-2` | o nó de tempo/pitch com as faixas declaradas — tempo 0,5 a 2,0×, pitch de −12 a +12 semitons, velocidade independente — e os criativos com reverb de mix 0–100% e decay 0,2–8 s, delay de 1 a 2000 ms com feedback 0–95%, chorus e flanger com rate 0,05–10 Hz, profundidade e mix | cada parâmetro é aplicado no mínimo, no meio e no máximo e o snapshot aceito é lido a cada passo; em seguida são submetidos tempo 0,4× e 2,5×, pitch +13 semitons, reverb com decay 0 s, delay com feedback 120% e o texto `abc` no campo de mix, e o snapshot e a mensagem de recusa são lidos | parameters validos |
| `TASK-E04-08-AC-3` | um par de faixas em crossfade de 0,5 s com reverb de decay 8 s e delay de 2000 ms ligados, tocando a 48 000 Hz com blocos de 64 frames, e os valores de `latency_frames()` e `tail_frames()` registrados antes do evento | um seek é disparado no meio da janela de crossfade, cancelando a geração anterior, e no bloco seguinte os nós de tempo, pitch e criativos são postos em bypass; cada amostra dos blocos seguintes é varrida atrás de NaN e Inf e a latência e o tail são lidos de novo depois do `reset` | cancel/bypass não produz NaN e latência indicada. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --test timefx --locked`, com `tests/fixtures/timefx/` recebendo por cópia `st16-44k1.flac` (44 100 Hz, 2 canais, 4 410 frames) e um episódio de fala gerado no próprio teste, mais um tom de 1 kHz e um impulso; a evidência é um CSV por caso com frames de entrada, frames entregues, cruzamentos de zero por segundo, `latency_frames()` e `tail_frames()` | Esperado: com a velocidade de podcast em 1,5×, a música entrega 4 410 frames para 4 410 frames de entrada (razão 1,0, nenhuma herança) enquanto o episódio entrega 2 940; a 2,0× os 4 410 frames saem em 2 205 ± 1 frame e a 0,5× em 8 820 ± 2; pitch de +12 semitons leva o tom de 1 000 Hz a 2 000 Hz ± 1% e mantém a contagem de frames, e −12 semitons a 500 Hz ± 1%; todo parâmetro aceito fica dentro da faixa declarada no snapshot e todo valor inválido é recusado com a faixa na mensagem, com o valor anterior mantido; após o seek com reverb e delay ativos, 0 NaN e 0 Inf nas amostras seguintes, `tail_frames()` volta a 0 no `reset` e `latency_frames()` não muda de valor antes e depois. Hoje nada disso existe: `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, 6 linhas, só `pub const VERSION`), nenhum `tests/`, `cargo test --locked` roda 0 testes em 2 suítes, e não há fixture de fala nem de podcast em nenhum repositório — o episódio do AC-1 precisa ser sintetizado no teste. |
| Hardware | nenhum: tempo, pitch, cancelamento e criativos são medidos por contagem de frames, cruzamentos de zero e varredura de NaN em buffers fora de linha, sem abrir dispositivo de áudio; este ambiente Windows 11 não tem DAC profissional, aparelho Android, aparelho iOS nem macOS, e nada disso é exigido aqui. | ausência é limitação registrada, não aprovação |

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
