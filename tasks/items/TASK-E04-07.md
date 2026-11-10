# TASK-E04-07 — Entregar dynamics, tone e channel controls

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E04-07` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E04-07` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E04-04` |
| Bloqueia | `TASK-E04-08`, `TASK-E04-10`, `TASK-E17-07` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–15 dias úteis |
| Risco | alto |

## Entrega

Limiter/compressor/bass/treble/mono/routing com meters.

## Boundary

- **Não** é a proteção final true-peak. O limiter aqui é criativo e reordenável dentro do segmento por stream; o guard de posição fixa, depois do master e do resampler e antes da quantização, é `TASK-E17-07`, e o teto medido no PCM quantizado do backend é `TASK-E17-08`.
- **Não** implementa o mapeamento de canais canônico nem a matriz 5.1/7.1. `TASK-E01-09` entrega os mapas mono, estéreo, 5.1 e 7.1; esta TASK consome a matriz e prova o headroom e o clipping, sem definir os coeficientes nem alegar canal derivado como canal real.
- **Não** faz resampling, dither nem conversão de bit depth. A lib própria de resampling e o packing PCM24/32 são `TASK-E04-02`; o estágio depois deste nó continua sendo um só resampler seguido do guard fixo.
- **Não** entrega ReplayGain, R128 nem medidor de LUFS. A análise de loudness e a escrita de tag são `TASK-E08-03` e o playback de RG é `TASK-E04-01`; limiter com headroom não é normalização, e misturar os dois aplicaria ganho duas vezes.
- **Não** constrói os medidores e o espectro da UI. `TASK-E06-05` é a dona dos medidores e do espectro; aqui se entrega o número por estágio — redução de ganho, pico, dBTP, contador de clipping — que a UI vai ler do snapshot.
- **Não** entrega o EQ paramétrico nem o preset que carrega a cadeia. `TASK-E04-04` é a dona do PEQ32, do schema e da precedência; esta TASK depende dela e só declara unidade, faixa e default de cada parâmetro no contrato de nó de `TASK-E04-03`.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e04-07-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E04-07-AC-1` | o limiter criativo com teto de −1 dBTP, lookahead de 96 frames a 48 000 Hz, oversampling 4× pinado e canal-link ligado, mais o compressor com threshold −18 dBFS, ratio 4:1, knee 6 dB, attack 10 ms, release 100 ms e makeup 0 dB, e uma fixture gerada no próprio teste — senoide a Fs/4 defasada de 45 graus com amplitude 0,5, pico amostral 0,3536 e pico verdadeiro exato 0,5, isto é −6,0206 dBTP | o sinal atravessa limiter e compressor com o ganho de entrada subindo até +12 dB, o pico verdadeiro do buffer entregue é estimado por um oráculo independente, a latência é medida por impulso na entrada do nó, e em seguida o nó é posto em bypass no meio da reprodução | True-peak fixture±0.1dB, latency publicada e bypass |
| `TASK-E04-07-AC-2` | o `sine-6ch-s24le.wav` de `libs/decode/tests/fixtures/pcm` (48 000 Hz, 6 canais, 4 800 frames, SHA-256 `560e0d390a8282bcefe95456629805e1d11edb0ef9f80e8da7958fef21067937`), cujos seis canais vêm do mesmo par de senoidais em fase, e a matriz de downmix 6→2 fornecida por `TASK-E01-09` com os pesos por canal e o pré-ganho de headroom declarados | o roteamento aplica a matriz, o buffer estéreo resultante é medido em pico amostral e em dBTP antes e depois do limiter, e o contador de clipping por estágio é lido nas duas posições, com o limiter ligado e em bypass | multichannel downmix com headroom e clipping explicitado. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --test dynamics --locked`, com a fixture de true peak gerada no próprio teste (senoide a Fs/4 defasada de 45 graus, pico verdadeiro analítico 0,5 = −6,0206 dBTP) e o `sine-6ch-s24le.wav` copiado para `tests/fixtures/dynamics/`, mais o estimador de referência em `tools/truepeak_ref.py` — Python puro com `math`, interpolação por janela de sinc de 4× com as mesmas fases, escrito à parte do código sob teste | Esperado: dBTP medido no buffer entregue ≤ −0,9 dBTP (teto −1 dBTP mais a tolerância de 0,1 dB que QA-027 fixa) com entrada até +12 dB de ganho, erro do estimador ≤ 0,1 dB contra os −6,0206 dBTP analíticos da fixture antes do limiter não atuar, latência medida por impulso igual a `latency_frames()` declarado — 96 frames de lookahead, 0 frame de diferença — e `tail_frames()` igual a 0, bypass com 0 amostra diferente da entrada e redução de ganho voltando a 0 dB no meter; no downmix, a soma medida dos seis canais da fixture chega a 3,0 de escala cheia contra o pico de 0,5 de um canal só — 9,54 dBFS, ou seja 15,56 dB acima daquele 0,5 — e por isso o headroom precisa ser maior que 0 dB antes de a matriz rodar; o pico estéreo reportado tem que fechar com a aritmética da matriz e com o pré-ganho declarado, e o contador de clipping fica em 0 com o limiter ligado e acima de 0 com ele em bypass, sem correção silenciosa. Hoje nada disso existe: `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, 6 linhas, só `pub const VERSION`), nenhum `tests/`, `cargo test --locked` roda 0 testes em 2 suítes, e não há fixture de true peak em nenhum repositório — ela precisa ser gerada no teste, como descrito. |
| Hardware | nenhum: limiter, compressor e downmix são medidos em amostras geradas no teste e no `sine-6ch-s24le.wav`, sem abrir dispositivo; o teto no PCM quantizado entregue ao backend é de `TASK-E17-08` e a matriz de hardware com DAC USB em 16/44,1, 24/96 e 24/192 é de `TASK-E04-10`, que este ambiente Windows 11 não tem — há só áudio integrado, sem placa pro, sem Android, sem iOS e sem macOS. | ausência é limitação registrada, não aprovação |

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
