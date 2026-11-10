# TASK-E04-01 — Implementar EQ10, ganho e ReplayGain playback

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E04-01` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E04-01` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M1 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E01-05` |
| Bloqueia | `TASK-E04-03`, `TASK-E08-03` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 3–8 dias úteis |
| Risco | alto |

## Entrega

EQ gráfico clássico, preamp, volume/pan e RG track/album.

## Boundary

- **Não** entrega EQ-20, EQ-31, EQ-64 nem o paramétrico. Esta TASK é o EQ gráfico de
  dez bandas (`31,25 Hz … 16 kHz`, `Q ≈ 1,41`), o preamp, volume/pan e ReplayGain
  track/album; os outros modos são `TASK-E04-04`, e o EQ-64 continua **M5**
  (`TASK-E12-10`), sem antecipação.
- **Não** constrói a UI do EQ. A curva calculada, os sliders e o painel clássico são
  `TASK-E02-03` (M1) e `TASK-E02-07` (M3). Aqui só existe o estado e o processamento; um
  controle manipulável na tela não é evidência desta TASK.
- **Não** define o crossfade nem a interpolação de parâmetro. Ligar, desligar, pôr em flat
  e trocar coeficiente no meio da reprodução usam o swap de `TASK-E04-03`; esta TASK
  produz coeficientes estáveis fora da callback e recebe a fronteira de bloco.
- **Não** analisa loudness nem escreve tag ReplayGain. Ler `REPLAYGAIN_TRACK_GAIN` e
  `REPLAYGAIN_ALBUM_GAIN` e aplicar o ganho é desta TASK; medir e gravar a tag é um job
  persistido de `TASK-E08-03`, com a análise versionada dele.
- **Não** lida com ReplayGain de Opus/USAC nem com gain/DRC de codec. O ganho intrínseco
  declarado, o priming e o preskip vêm do `vynamp-decode` (`TASK-E17-02`), e o teste de
  não aplicar ganho duas vezes é da cadeia, não do nó de EQ.
- **Não** faz o perfil por dispositivo nem o import de headphone preset. O preamp é
  guardado por modo; resolver precedência sessão > faixa > gênero > dispositivo > global
  e importar AutoEQ com origem e licença é `TASK-E04-09`.
- **Não** é a proteção final true-peak. O EQ pode somar boosts e saturar; o guard depois do
  master e antes da quantização é `TASK-E17-07`, e ele não corrige filtro instável — neste
  nó, coeficiente instável é recusado antes do swap.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e04-01-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E04-01-AC-1` | o EQ de dez bandas com o perfil de dispositivo carregado | o EQ é ligado, desligado e posto em flat, e a resposta em frequência de cada estado é medida | On/off/flat coerentes |
| `TASK-E04-01-AC-2` | uma faixa sem tags ReplayGain e o preamp configurado | o playback aplica ReplayGain track e album e a saída é medida contra a referência | metadata absent usa ganho configurado |
| `TASK-E04-01-AC-3` | o EQ tocando sob a carga de callback do spike de E01-02 | os coeficientes são trocados pelo soquete de controle no meio da reprodução | coefficient swap sem zipper e sem allocRT. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine`, mais os testes `EQ-T-04` e `EQ-T-11` de `specs/equalizadores.md` §16: impulso/sweep por banda a ±6 e ±15 dB em 44,1/48/96/192 kHz, e alternar On/Off/flat sob tom de 1 kHz com detector de descontinuidade | verde no CI Gate, com `EQ-T-04` dentro de ±0,2 dB nos pontos definidos, sem NaN e estável perto de Nyquist, e `EQ-T-11` sem degrau acima do limiar — a comparação é numérica, não auditiva |
| Oráculo independente | resposta da cascata calculada fora do código sob teste, pela fórmula do §4 (`f_k = 1000 · 2^(k−5)`, `Q ≈ 1,41`) e do §8 (`H(f) = G_preamp · ∏ H_k(e^{j2πf/Fs})`), avaliada nos mesmos pontos do dump de coeficientes e de resposta do nó; para ReplayGain, o ganho esperado somado à mão a partir do valor da tag mais o preamp configurado | a resposta medida bate a calculada dentro da tolerância de `EQ-T-04` em cada banda e estado, e a saída de uma faixa sem tag é a de entrada vezes o preamp configurado, sample a sample — dois números comparados, não "parece certo" |
| Hardware | necessário para a metade que mede áudio: a resposta em frequência de AC-1 e AC-2 só fecha com dispositivo ou backend de captura; o harness de callback do spike de `TASK-E01-02` (WASAPI shared, 384 000 Hz, buffer 3 840 frames) é a referência de carga para AC-3, e aqui não há dispositivo algum | ausência é limitação registrada, não aprovação: sem backend, o teste cobre coeficientes e ganho por amostra, não a saída analógica nem o zipper no dispositivo |

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
