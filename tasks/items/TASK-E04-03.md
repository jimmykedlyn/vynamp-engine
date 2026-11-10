# TASK-E04-03 — Construir grafo DSP e transições sem glitches

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E04-03` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E04-03` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M2 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E04-01`, `TASK-E04-02` |
| Bloqueia | `TASK-E04-04`, `TASK-E04-06`, `TASK-E04-08`, `TASK-E10-03`, `TASK-E17-07`, `TASK-E19-03`, `TASK-E19-07` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–12 dias úteis |
| Risco | alto |

## Entrega

Compiled graph, parameter interpolation, latency annotation e crossfade.

## Boundary

- **Não** entrega nenhum equalizador. O EQ-10, o preamp, o volume/pan e o ReplayGain são `TASK-E04-01`, e o EQ-20, EQ-31 e o PEQ32 são `TASK-E04-04`; um nó de EQ no grafo de teste é fixture, não entrega, e trazer a resposta de filtro para cá faria dois donos da mesma tolerância de ±0,2 dB.
- **Não** implementa o resampler nem decide onde ele senta na cadeia. A lib própria de resampling, o packing PCM24/32 e a qualidade de aliasing são `TASK-E04-02`; aqui só existe a anotação de latência que o nó do resampler declara quando ele existir.
- **Não** é a proteção final true-peak. O teto fixo depois do master e antes da quantização é `TASK-E17-07`, e o grafo não pode colocar ganho, plugin, mix ou resampling depois dele; o limiter do grafo de teste é criativo e reordenável.
- **Não** constrói o oráculo numérico de FFT, resposta em frequência, delay/padding ou detector de descontinuidade. `TASK-E10-03` é a dona desse harness e o consome a partir daqui — o backlog a declara dependente desta TASK; implementar o detector neste repositório faria o crossfade testar a si mesmo.
- **Não** faz o mixer de fontes, o dreno de EOF nem o ramp de play/stop. Soma de streams PCM e o último frame de conteúdo chegando ao backend são `TASK-E01-05`; o grafo troca de geração, quem garante que nada se perde no fim do stream é ela.
- **Não** integra o vitrola nem qualquer emulação analógica como nó. `TASK-E19-07` é a dona do módulo interno de vinil e válvula, e `TASK-E19-03` dissolve o desenho; aqui só existe o registro de nós e o contrato que o adaptador vai consumir.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e04-03-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E04-03-AC-1` | um grafo de quatro nós (`preamp` em 0 dB, `peaking` a 1 kHz com Q 4,32 e ganho +3 dB, `compressor` com attack 10 ms e `limiter` com lookahead 96 frames) compilado para 48 000 Hz, blocos de 64 frames, consumido por uma thread de áudio falsa que só chama `process` sobre `sine-s24le.wav` de `libs/decode/tests/fixtures/pcm` (48 000 Hz, 2 canais, 4 800 frames, SHA-256 `4c7476c605ffde493009fd9cca3e0403d5dc8e053ebf33ba1fefe7b7f855f4cc` do `MANIFEST.txt` de lá), com o `CountingAlloc` de `libs/output/crates/vynamp-output-testkit/src/alloc.rs` declarado como allocator global do binário de teste | a thread de controle publica um snapshot novo — `peaking` em −6 dB e um nó `reverb` acrescentado no meio da cadeia — enquanto a thread de áudio continua consumindo, e ao fim cada bloco é classificado pela geração de origem, o contador de alocações é lido e a soma das latências declaradas é comparada com a `latency_frames()` do grafo compilado | Graph swap foraRT |
| `TASK-E04-03-AC-2` | um grafo declarado com 12 nós e 14 conexões, contendo uma conexão repetida entre o mesmo par de nós, uma autoconexão e um ciclo fechado de três nós, mais um ramo válido em que um `biquad` a 2 kHz recebe ganho entre −30 e +40 dB a 48 000 Hz | o grafo é compilado e, no caminho aceito, o sinal atravessa 10 000 blocos de 64 frames com o parâmetro do `biquad` varrido nos extremos, cada amostra do buffer de saída é inspecionada e o diagnóstico de compilação do grafo inválido é lido | no cycles/NaN |
| `TASK-E04-03-AC-3` | duas faixas de 96 000 frames a 48 000 Hz com o mesmo conteúdo correlacionado, a política de crossfade equal-power de 0,5 s (24 000 frames) ligada, e o par de faixas do teste de gapless de `TASK-E01-06` como segunda fixture | o crossfade é aplicado e a saída é medida em frames totais, no envelope de ganho de cada fonte durante a janela de 24 000 frames e no detector de descontinuidade; depois o crossfade é desligado e o par de gapless é emendado de novo | equal-power crossfade preserva duração, desligado não altera gapless. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --test graph --locked`, com as fixtures em `tests/fixtures/graph/` copiadas de `libs/decode/tests/fixtures/pcm` (`sine-s24le.wav` e `sine-6ch-s24le.wav`, 48 000 Hz, 4 800 frames cada, SHA-256 conferido contra o `MANIFEST.txt` de lá), o `CountingAlloc` de `libs/output/crates/vynamp-output-testkit/src/alloc.rs` como allocator global do binário de teste, e `examples/graph_swap.rs` publicando snapshots enquanto a thread falsa consome blocos de 64 frames | Esperado: 0 alocações na thread de áudio durante os 10 000 blocos (o contador do `CountingAlloc` só sobe antes do primeiro `process` e depois do último), 0 blocos misturando duas gerações — o swap cai sempre na fronteira de 64 frames —, a soma das latências declaradas igual à `latency_frames()` do grafo (dois ramos paralelos de 512 frames reportam 512, não 1 024, e o `limiter` acrescenta os seus 96), 0 ciclo aceito e as três declarações inválidas recusadas com motivo nomeado, 0 NaN e 0 Inf em 640 000 amostras com o `biquad` varrido até +40 dB, e no crossfade a soma dos quadrados dos ganhos igual a 1 dentro de 1e-12 por toda a janela, 168 000 frames de saída para 96 000 + 96 000 − 24 000 (0 frame a mais ou a menos), e com o crossfade desligado a saída bit-idêntica à concatenação do par de gapless de `TASK-E01-06`. Hoje nada disso existe: `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, 6 linhas, só `pub const VERSION`), `Cargo.toml` sem nenhuma dependência, nenhum diretório `tests/` nem `examples/`, e `cargo test --locked` roda 0 testes em 2 suítes — `cargo test --test graph` falha por alvo inexistente e não prova nenhum dos números acima. |
| Hardware | nenhum: o swap é medido numa thread de áudio falsa que consome blocos em memória e no contador de alocação, sem abrir dispositivo; a referência de carga real é a callback de WASAPI shared a 384 kHz com buffer de 3 840 frames medida no spike de `TASK-E01-02`, que fica registrada como expectativa e não é remedida aqui. Este ambiente é Windows 11 sem placa de áudio profissional, sem Android, sem iOS e sem macOS. | ausência é limitação registrada, não aprovação |

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
