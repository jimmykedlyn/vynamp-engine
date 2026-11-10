# TASK-E12-10 — Completar EQ64 e escala avançada

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E12-10` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E12-10` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M5 |
| Prioridade | P2 |
| Estado | planned |
| Depende de | `TASK-E04-10`, `TASK-E03-10`, `TASK-E12-03` |
| Bloqueia | nenhuma |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 10–25 dias úteis |
| Risco | alto |

## Entrega

EQ64, bandas por canal, surround opcional, biblioteca de 1 milhão e RTL completo.

## Boundary

- **Não** constrói o editor de 64 bandas nem o inspetor de cadeia na UI: `TASK-E02-07` é a dona do audio-chain inspector, do editor de curva e bandas e dos device profiles. Aqui se entrega processamento e números; o teste de 64 controles reais editáveis é da UI, e `TASK-E02-09` e `TASK-E02-10` fecham localização e jornada acessível.
- **Não** entrega a resposta numérica do EQ-20, EQ-31 nem do PEQ32 dentro de ±0,2 dB: `TASK-E04-04` é a dona desses três modos e do import de preset. Trazer a validação deles para cá faria o mesmo crítico de ±0,2 dB ter dois donos e dois relatórios.
- **Não** implementa schema, migrations, FTS nem paginação da biblioteca: `TASK-E03-01` é a dona do schema e do write actor e `TASK-E03-05` da query port indexada. O alvo de 1 milhão é medido sobre o que elas expõem, e o relatório de 100k continua sendo o de `TASK-E03-10`.
- **Não** implementa o mapeamento de canais nem o downmix com headroom: `TASK-E01-09` entrega os mapas mono, estéreo, 5.1 e 7.1. O que esta TASK deve é o rótulo honesto da emulação e a garantia de não apresentar canal derivado como canal real.
- **Não** implementa o algoritmo de voice remover nem promete separação de fontes: `TASK-E04-08` é a dona dos efeitos criativos e da entrada exploratória de voice remover. Aqui o item é a explicitação do estado experimental, não o DSP dele.
- **Não** constrói a instrumentação de métricas nem o baseline que os números comparam: `TASK-E10-07` entrega o histograma de callback, as métricas sanitizadas e o baseline em JSON, e `TASK-E10-03` o harness numérico. Esta TASK mede e publica contra esses instrumentos.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e12-10-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E12-10-AC-1` | um build Release do engine com o EQ64 de 64 bandas por canal em cascata f64 e preamp em 0 dB, o sinal `libs/decode/tests/fixtures/pcm/sine-f32le.wav` reamostrado para 44,1, 96, 192 e 384 kHz, e um banco sintético com 1 milhão de itens no schema de `TASK-E03-01` | roda-se o benchmark de callback por pelo menos 1M invocações registrando p95 e p99.9 do período negociado com runs maiores ou iguais a 30 e processo aquecido, conforme o protocolo de `quality/02-performance.md`, e sobre o banco de 1 milhão mede-se busca warm e restauração de fila | Medir CPU/latência e biblioteca |
| `TASK-E12-10-AC-2` | uma fonte estéreo de 2 canais e a emulação surround habilitada com saída em 5.1 pelo layout de `TASK-E01-09`, tendo `libs/decode/tests/fixtures/pcm/sine-6ch-s24le.wav` como referência de 6 canais reais | o painel DSP, o inspetor de cadeia e o `.vyndiag` exportado são lidos com a emulação ativa, e a captura por canal confirma quais canais são derivados e quais são conteúdo original | emulação surround rotulada |
| `TASK-E12-10-AC-3` | o editor nativo de 64 bandas com os 64 sliders reais, a árvore semântica exposta pelo toolkit, o locale forçado a um idioma de escrita da direita para a esquerda e o Narrator do Windows 11 ligado | percorre-se o editor inteiro só com teclado (Tab, Shift+Tab, setas, Home e End, mais o campo numérico de cada banda), depois com o leitor de tela anunciando nome, frequência e dB de cada banda, e por fim com a interface espelhada e fonte escalada a 200% | RTL/a11y validados |
| `TASK-E12-10-AC-4` | o módulo de voice remover exploratório registrado no catálogo DSP com estado declarado experimental, e um preset que o habilita junto do EQ64 | o módulo é ativado durante o playback e o usuário lê o rótulo no painel, o estado no preset exportado, o aviso da primeira ativação e o texto do `.vyndiag` | voice remover experimental explicitado. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cargo test --release --package vynamp_engine --test eq64_response --locked` comparando a cascata de 64 biquads com o cálculo independente da resposta em 512 pontos logarítmicos de 10 Hz até o mínimo entre 40 kHz e Fs/2, `cargo test --release --test eq64_bench --locked` com histograma de callback por pelo menos 1M invocações a 44,1, 96, 192 e 384 kHz, `cargo test --test surround_label --locked` sobre `sine-6ch-s24le.wav`, e o gate de 1M com `cargo test --test library_1m --locked` mais o relatório de bench no formato JSON de `quality/02-performance.md` | esperado: curva medida contra a calculada dentro de ±0,2 dB (QA-005) nos pontos da grade de `reference/audio/08-equalizers.md` §7, sem NaN e com a banda 64 em 20 kHz marcada indisponível a 44,1 kHz pelo limite de 0,45 × Fs, e banda 62 a 64 a 32 kHz; p99.9 da callback abaixo de 25% do período negociado e busca warm p95 menor ou igual a 100 ms com 1 milhão de itens. Hoje, medido em 2026-11-10, isso não existe: `cargo test --all --locked` no engine roda 0 testes em 2 suítes e `src/lib.rs` tem 6 linhas com só `pub const VERSION`; não há dataset de 1 milhão (o gate escrito é 100k, em `TASK-E03-10`), nem harness numérico (`TASK-E10-03` planejada), nem instrumentação de histograma (`TASK-E10-07` planejada), nem fixture de 8 canais para 7.1 em `libs/decode/tests/fixtures/pcm`, e `specs/equalizadores.md` §5 está vazia, então nenhuma TASK deste módulo pode sair de `planned`. |
| Hardware | nenhum: CPU, latência, curva e biblioteca são medidos offline em captura por canal e em banco sintético, sem dispositivo de áudio. As duas partes que pediriam hardware não têm: não há saída 5.1 ou 7.1 real, só o áudio Intel e Conexant integrado deste Windows, e não há NVDA nem Narrator automatizável instalados; a leitura com leitor de tela e a escuta multicanal ficam como limitação declarada, não como aprovação, e a matriz de hardware por dispositivo continua em `TASK-E04-10`. | ausência é limitação registrada, não aprovação |

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
