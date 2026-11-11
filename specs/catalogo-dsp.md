# Spec — vynamp-engine · catalogo-dsp

| Campo | Valor |
|---|---|
| Módulo | `vynamp-engine` |
| Crate(s) | `vynamp-engine` (módulo `catalogo`; crate único até uma TASK justificar divisão) |
| Tipo | lib |
| Versão da spec | 0.2.0 |
| Data | 2026-11-11 |
| Estado | **rascunho autorado, não aprovado** — §3 a §6, §9 e §10 escritos a partir de `reference/audio/09-dsp-catalog.md` e das TASKs E04-07 e E04-08. A aprovação é do mantenedor, por `GOVERNANCE.md` |
| Dono | mantenedor do `vynamp-engine` |
| Depende de | decode, encode, output, dsp (as quatro) |
| É dependido por | player, converter, restore |

> §1, §2 e §7 foram decididos em `01-arvore-final.md` §2 e valem como norma. §3 a §6,
> §9 e §10 foram autorados em 2026-11-11 a partir do material de referência de áudio e
> das TASKs citadas. Estar escrito não é estar aprovado: até o mantenedor aprovar,
> nenhum número aqui é contrato, e toda TASK que dependa dele continua `planned`.
> Esta spec fixa os módulos do catálogo DSP que ocupam os slots `Dynamics`, `Spatial` e
> `Creative`: compressor, limiter criativo, tom (bass/treble), balance/mono, matriz de
> canais, tempo/pitch e efeitos criativos. O EQ e o preamp são de `equalizadores.md`.
> O contrato de nó, o bypass, o smoothing, a ordem de slots e os presets são de
> `dsp.md`; aqui só se diz o que cada nó calcula e quais faixas aceita.

## 1. Propósito

Integrar decode, dsp, output e encode num grafo de reproducao com estado, comandos e grafo DSP.

**Não faz:** Nao tem UI, nao gerencia biblioteca, nao abre dispositivo por conta propria sem o output.

## 2. Fronteira

| Permitido | Proibido |
|---|---|
| herda a fronteira de `specs/engine.md` | herda as proibicoes de `specs/engine.md` |

## 3. API pública

> Autorado em 2026-11-11 e **não aprovado**. Os tipos abaixo são a forma proposta;
> nomes podem mudar na TASK, o comportamento descrito em §4 não.

### 3.1 Regras comuns a todo módulo do catálogo

Todo módulo daqui é um `Node` de `dsp.md` §3 e herda dele, sem exceção:

- bypass real com crossfade (NODE-REQ-04);
- smoothing de parâmetro de 10 a 50 ms (NODE-AC-02);
- `latency_frames()` e `tail_frames()` declarados e medidos, nunca estimados;
- parâmetros serializáveis e versionados no preset (`dsp.md` §3.5);
- bypass seguro em `NaN`, `Inf` ou instabilidade, com `NodeError::NonFinite` (NODE-REQ-08);
- estado exposto como `ModuleStatus`:

```rust
pub enum ModuleStatus {
    Active,
    Inactive,
    Bypassed,
    Incompatible { reason: String },   // ex.: layout de canais não suportado
    Error { reason: String },
}
```

A cauda (`tail_frames() > 0`) é drenada ou cortada pela política de stop/seek/troca de
faixa de `pipeline.md`; o nó só declara a cauda e zera o estado em `reset()`.

### 3.2 Medidores por estágio

```rust
pub struct StageMeters {
    pub gain_reduction_db: f32,   // 0,0 quando não há redução ou em bypass
    pub sample_peak_dbfs: f32,
    pub true_peak_dbtp: f32,      // oversampling 4×
    pub clip_count: u64,          // amostras com |x| > 1,0 na saída do estágio
}

pub trait Metered {
    fn meters(&self) -> StageMeters;
}
```

Os números saem do callback por fila sem lock; o desenho dos medidores é da UI (E06-05).

### 3.3 Dinâmica e tom (slot `Dynamics`) — E04-07

| Nó | Parâmetro | Faixa | Padrão |
|---|---|---|---|
| `Compressor` | threshold | −60 a 0 dBFS | −18 |
| | ratio | 1:1 a 20:1 | 4:1 |
| | knee | 0 a 24 dB | 6 |
| | attack | 0,1 a 200 ms | 10 |
| | release | 5 a 2000 ms | 100 |
| | makeup | 0 a +24 dB | 0 |
| `Limiter` (criativo) | ceiling | −12 a 0 dBTP | −1 |
| | lookahead | fixo: 2 ms (96 frames a 48 kHz) | — |
| | oversampling | fixo: 4× | — |
| | channel link | ligado / desligado | ligado |
| `Tone` | bass (shelf 100 Hz) | ±12 dB | 0 |
| | treble (shelf 10 kHz) | ±12 dB | 0 |

O `Limiter` daqui é criativo e fica no slot `Dynamics`. **Não** é a proteção final de
true-peak da saída, que é de E17-07/E17-08 e roda depois de `VolumePan`.

### 3.4 Canais (slot `Spatial`) — E04-07, com mapas de E01-09

| Nó | Parâmetro | Faixa | Padrão |
|---|---|---|---|
| `Balance` | balance | −1,0 a +1,0 | 0 |
| | mono | ligado / desligado | desligado |
| `ChannelMatrix` | matriz `out × in` de pesos | −1,0 a +1,0 por célula | identidade |
| | headroom (pré-ganho) | −24 a 0 dB | calculado (ver abaixo) |

A matriz é carregada de JSON:

```json
{ "version": 1, "in": 6, "out": 2,
  "weights": [[1.0, 0.0, 0.7071, 0.7071, 0.7071, 0.0],
              [0.0, 1.0, 0.7071, 0.7071, 0.0, 0.7071]],
  "headroom_db": null }
```

Com `headroom_db: null`, o pré-ganho é `−20·log10(máx_linha Σ|w|)`, o que garante que
a soma nunca passa de 1,0 com entrada em fundo de escala. O layout de canais em si
(ordem e nomes) é de E01-09; aqui só se aplica a matriz.

A lei de pan do `Balance` é **decisão aberta** (§10); até ser fixada, vale a lei de
balance linear (o canal oposto atenua, o lado escolhido não ganha). O pan equal-power
do `VolumePan` é de `equalizadores.md` e não muda.

### 3.5 Tempo, pitch e efeitos (slot `Creative`) — E04-08

| Nó | Parâmetro | Faixa |
|---|---|---|
| `TimeStretch` | tempo | 0,5× a 2,0× |
| | pitch | −12 a +12 semitons |
| `Reverb` | mix | 0 a 100 % |
| | decay | 0,2 a 8 s |
| `Delay` | tempo | 1 a 2000 ms |
| | feedback | 0 a 95 % |
| | mix | 0 a 100 % |
| `Chorus`, `Flanger` | rate | 0,05 a 10 Hz |
| | depth | 0 a 100 % |
| | mix | 0 a 100 % |
| `SilenceTrim` | limiar | −90 a −30 dBFS |
| | duração mínima | 100 a 5000 ms |

`TimeStretch` muda o número de frames: `frames_out = round(frames_in / tempo)`. O pitch
sozinho não muda o número de frames. A velocidade é **por faixa**: ela é aplicada à
faixa que a pediu e volta a 1,0× na seguinte, salvo regra explícita do player
(velocidade por categoria é de E08-09).

```rust
pub struct TempoRequest { pub track: TrackId, pub tempo: f64, pub pitch_semitones: f64 }
```

### 3.6 Módulos propostos, sem TASK

Os módulos abaixo constam de `reference/audio/09-dsp-catalog.md` mas **não têm TASK nem
marco**: Dynamic EQ, Loudness Compensation, Bass Enhancer, Exciter, Convolution, Room
Correction, Multiband, Declipper, Crossfeed, Width, M/S, Phase/Delay, Bass Management e
HRTF. Esta spec não fixa nada sobre eles. Cada um entra por TASK própria, que acrescenta
a linha em §3 e os requisitos em §4. O removedor de voz é exploratório (M5) e fica no
slot `Spatial` quando existir.

### 3.7 Famílias do painel

O painel agrupa os módulos em oito famílias de navegação: Ganho/EQ, Dinâmica, Acústica,
Estéreo, Tempo/formato, Criativos, Vitrola e Saída. Os medidores são uma área à parte.
A família é metadado do nó (`fn family(&self) -> Family`), não muda o slot.

## 4. Requisitos

| ID | Requisito | REQ de produto | TASK | Onde é provado |
|---|---|---|---|---|
| `CAT-REQ-01` | Todo módulo do catálogo cumpre as regras comuns de §3.1 e expõe `ModuleStatus` — planned (M3) | `REQ-E04-07`, `REQ-E04-08` | `TASK-E04-07`, `TASK-E04-08` | — (CAT-AC-01, CAT-AC-06 planejados) |
| `CAT-REQ-02` | O `Limiter` com teto −1 dBTP mantém o true-peak medido ≤ −0,9 dBTP com +12 dB de ganho de entrada; latência medida = `latency_frames()` = 96 a 48 kHz; `tail_frames()` = 0 — planned (M3) | `REQ-E04-07` | `TASK-E04-07` | — (CAT-AC-01 planejado) |
| `CAT-REQ-03` | O `Compressor` aplica threshold, ratio, knee, attack, release e makeup de §3.3, e reporta a redução de ganho — planned (M3) | `REQ-E04-07` | `TASK-E04-07` | — (CAT-AC-02 planejado) |
| `CAT-REQ-04` | Em bypass, a saída de cada nó de `Dynamics` é idêntica à entrada e a redução de ganho reportada é 0 dB — planned (M3) | `REQ-E04-07` | `TASK-E04-07` | — (CAT-AC-01 planejado) |
| `CAT-REQ-05` | Todo estágio expõe `StageMeters` (redução de ganho, pico, dBTP, contador de clip) — planned (M3) | `REQ-E04-07` | `TASK-E04-07` | — (CAT-AC-01, CAT-AC-03 planejados) |
| `CAT-REQ-06` | A `ChannelMatrix` aplica a matriz de JSON com headroom que impede soma > 1,0; o pico estéreo bate com a aritmética da matriz — planned (M3) | `REQ-E04-07`, `REQ-E01-09` | `TASK-E04-07` | — (CAT-AC-03 planejado) |
| `CAT-REQ-07` | `Tone` e `Balance`/mono respeitam as faixas de §3.3 e §3.4 — planned (M3) | `REQ-E04-07` | `TASK-E04-07` | — (CAT-AC-04 planejado) |
| `CAT-REQ-08` | A velocidade é por faixa: a faixa seguinte toca a 1,0× salvo regra explícita — planned (M3) | `REQ-E04-08` | `TASK-E04-08` | — (CAT-AC-05 planejado) |
| `CAT-REQ-09` | Valores fora das faixas de §3.5, não finitos ou não numéricos são recusados com a faixa na mensagem, e o valor anterior fica — planned (M3) | `REQ-E04-08` | `TASK-E04-08` | — (CAT-AC-06 planejado) |
| `CAT-REQ-10` | `TimeStretch` produz `round(frames_in / tempo)` frames; pitch ±12 st desloca a frequência por `2^(st/12)` sem mudar o número de frames — planned (M3) | `REQ-E04-08` | `TASK-E04-08` | — (CAT-AC-07 planejado) |
| `CAT-REQ-11` | Caudas longas de reverb e delay sob crossfade, seek e bypass não geram `NaN`/`Inf`; `reset()` zera a cauda e não muda `latency_frames()` — planned (M3) | `REQ-E04-08` | `TASK-E04-08` | — (CAT-AC-08 planejado) |
| `CAT-REQ-12` | Módulo de §3.6 não aparece como opção ativa; aparece como "indisponível nesta versão" até ter TASK — planned (M3) | `REQ-E04-07` | `TASK-E04-07` | — (CAT-AC-09 planejado) |

Os nomes de teste em §5 são rótulos descritivos; o nome final é fixado pela TASK e
registrado na coluna "Onde é provado".

## 5. Critérios de aceite

| ID | Given | When | Then | Teste |
|---|---|---|---|---|
| `CAT-AC-01` | `Limiter` com teto −1 dBTP, lookahead 96 frames a 48 kHz, oversampling 4×, link ligado; fixture seno em Fs/4 com fase 45°, amplitude 0,5 (pico de amostra 0,3536, true-peak 0,5 = −6,0206 dBTP) | o ganho de entrada sobe a +12 dB, depois o nó vai a bypass | true-peak medido ≤ −0,9 dBTP (estimador com erro ≤ 0,1 dB); latência medida por impulso = `latency_frames()` = 96; `tail_frames()` = 0; em bypass, 0 amostras diferem da entrada e a redução de ganho é 0 dB | `limiter_holds_ceiling_and_latency` |
| `CAT-AC-02` | `Compressor` com threshold −18 dBFS, ratio 4:1, knee 6 dB, attack 10 ms, release 100 ms, makeup 0 dB; seno de 1 kHz em degraus de −30, −18, −6 e 0 dBFS | cada degrau estabiliza (≥ 5 × release) | o ganho de saída em regime bate com a curva estática com knee suave dentro de ±0,5 dB, e `gain_reduction_db` bate com a diferença entrada/saída dentro de ±0,1 dB | `compressor_static_curve` |
| `CAT-AC-03` | `sine-6ch-s24le.wav` (SHA-256 `560e0d390a8282bcefe95456629805e1d11edb0ef9f80e8da7958fef21067937`; 48 kHz, 6 canais, 4800 frames, pico 0,5) e `downmix-6to2.json` | a matriz 6→2 é aplicada com headroom calculado, uma vez com o `Limiter` ligado e outra com ele em bypass e sem headroom | a soma sem headroom chegaria a 3,0 (+9,54 dBFS), então o headroom aplicado é > 0; o pico estéreo bate com a aritmética da matriz; `clip_count` = 0 com limiter e > 0 em bypass sem headroom | `downmix_headroom_and_clip_count` |
| `CAT-AC-04` | `Tone` com bass +12 dB e treble −12 dB; `Balance` em −1, 0 e +1; mono ligado com L = −R | a resposta é medida por impulso e o seno estéreo toca | os shelves ficam em ±0,5 dB do RBJ calculado fora do código sob teste; em −1 o canal R é 0; em 0 nenhum canal muda; mono com L = −R dá saída ≤ −90 dBFS | `tone_and_balance_ranges` |
| `CAT-AC-05` | um episódio de podcast a 1,5× seguido de `st16-44k1.flac` (44,1 kHz, 2 canais, 4410 frames) | a fila avança para a faixa seguinte | o episódio gera 2940 frames por 4410 de entrada; a faixa seguinte entra com 4410 e sai com 4410 (1,0×) | `speed_is_per_track` |
| `CAT-AC-06` | estado válido conhecido de todos os nós de §3.5 | são aplicados tempo 0,4 e 2,5, pitch +13, decay 0, feedback 120 % e mix `"abc"` | cada um é recusado com a faixa na mensagem; o estado continua idêntico ao anterior | `invalid_creative_params_keep_previous` |
| `CAT-AC-07` | `st16-44k1.flac` (4410 frames) e um tom de 1 kHz | tempo 2,0× e 0,5×; depois pitch +12 e −12 st a tempo 1,0× | 2,0× dá 2205 ± 1 frames; 0,5× dá 8820 ± 2; +12 st leva 1 kHz a 2 kHz ± 1 %, −12 st a 500 Hz ± 1 %; com pitch o número de frames não muda | `tempo_and_pitch_arithmetic` |
| `CAT-AC-08` | crossfade de 0,5 s com `Reverb` decay 8 s e `Delay` 2000 ms feedback 95 %, a 48 kHz em blocos de 64 frames | um seek cai no meio do crossfade e em seguida os dois nós vão a bypass | 0 `NaN` e 0 `Inf` em toda a saída; após `reset()` `tail_frames()` volta a 0; `latency_frames()` não muda | `long_tails_survive_seek_and_bypass` |
| `CAT-AC-09` | a lista de módulos do catálogo num build de M3 | a lista é consultada | os módulos de §3.6 aparecem com `ModuleStatus::Incompatible { reason: "indisponível nesta versão" }` e não entram na cadeia | `proposed_modules_are_unavailable` |

**Nenhum destes testes existe ainda.** Os valores numéricos vêm de
`reference/audio/09-dsp-catalog.md` e dos AC de `TASK-E04-07` e `TASK-E04-08`.

### Critérios que esta spec **não** fixa

- A proteção final de true-peak da saída: é de E17-07/E17-08.
- Os mapas de canal (ordem, nomes, layouts além de 5.1): são de E01-09.
- Resampling e dither: são de E04-02.
- ReplayGain e LUFS: são de `equalizadores.md` (E04-01) e E08-03.
- Os medidores na UI e a velocidade por categoria: são de E06-05 e E08-09.
- O drenar da cauda e a rampa em stop/seek: são de E01-05 (`pipeline.md`).
- A ausência de clique na troca: smoothing e crossfade são de `dsp.md` (NODE-AC-02 e
  NODE-AC-06).

## 6. Erros e cancelamento

**Erro é dado, não `panic!`.** Falhas do catálogo usam um tipo próprio, convertido em
`NodeError` (`dsp.md` §6) quando atravessa o grafo:

```rust
pub enum CatError {
    OutOfRange { field: String, value: f64, min: f64, max: f64 },
    NonFinite { field: String },
    NotANumber { field: String, raw: String },
    MatrixShape { expected_in: usize, expected_out: usize, got_in: usize, got_out: usize },
    MatrixInvalid { pointer: String, reason: String },
    LayoutIncompatible { node: &'static str, channels: usize },
    ModuleUnavailable { module: &'static str },
}
```

| Situação | Efeito |
|---|---|
| valor fora de faixa, não finito ou não numérico | recusado; o valor anterior fica e o erro nomeia o campo e a faixa |
| matriz com dimensão diferente do layout de entrada | recusada com `MatrixShape`; a matriz vigente continua |
| JSON de matriz inválido | `MatrixInvalid` com JSON pointer |
| nó sem suporte ao número de canais | `ModuleStatus::Incompatible` e bypass; **não** para a reprodução |
| módulo de §3.6 pedido | `ModuleUnavailable` |
| saída `NaN`/`Inf` em runtime | bypass seguro com `NodeError::NonFinite` (`dsp.md`) |

Nenhuma dessas situações para a reprodução.

**Cancelamento é explícito, não timeout.** Recalcular coeficientes, alocar buffers de
lookahead ou de cauda e carregar uma matriz rodam fora do callback e são descartados
quando chega uma geração mais nova (`dsp.md` §6). O catálogo não tem prazo próprio.

## 7. Fora de escopo

Vinyl/Tube e VST3 entram como modulos internos deste grafo (E19-07, E12-07). Jobs persistidos (E08-01) entram como infraestrutura nao-audio, decisao registrada

## 8. Dependências

Depende de: decode, encode, output, dsp (as quatro)

É dependido por: player, converter, restore

## 9. Testes e fixtures

| Fixture | Origem | Uso |
|---|---|---|
| `tests/fixtures/dynamics/sine-6ch-s24le.wav` | cópia de `libs/decode/tests/fixtures/pcm`, SHA-256 conferido | CAT-AC-03 |
| `tests/fixtures/dynamics/downmix-6to2.json` | escrito pela TASK E04-07: pesos de §3.4 | CAT-AC-03 |
| seno Fs/4 com fase 45°, degraus de 1 kHz, impulso | sintetizados no próprio teste | CAT-AC-01, CAT-AC-02, CAT-AC-04 |
| `tests/fixtures/graph/st16-44k1.flac` | cópia de `libs/decode/tests/fixtures`, SHA-256 conferido | CAT-AC-05, CAT-AC-07 |
| `tests/fixtures/timefx/` | escrito pela TASK E04-08: episódio de fala sintetizado, tom de 1 kHz, impulso; um CSV de saída por caso | CAT-AC-05 a CAT-AC-08 |

O oráculo de true-peak é `tools/truepeak_ref.py` (Python puro, sinc janelado 4×), que
não usa o código sob teste. O oráculo de pitch é a frequência de pico da FFT com
interpolação parabólica. Nenhum teste depende de arquivo grande; não há `#[ignore]`.

```bash
cd libs/engine && cargo test --test dynamics --locked
cd libs/engine && cargo test --test timefx --locked
```

## 10. Limitações conhecidas

Limitação declarada não reprova a entrega; limitação omitida reprova.

| Limitação | Efeito | Quem cobre |
|---|---|---|
| Lei de pan do `Balance` é decisão aberta | vale balance linear até a decisão; trocar depois muda o nível percebido fora do centro | decisão do mantenedor; registrar em ADR |
| `Limiter` criativo não é proteção de saída | um preset sem limiter pode clipar antes do guarda final | E17-07/E17-08 |
| Qualidade do time-stretch em fala vs. música não é medida | 2,0× em música densa pode ter artefatos audíveis; só a aritmética de frames é contrato | E04-08 declara; avaliação de escuta sem TASK |
| Removedor de voz é exploratório | não existe antes de M5 | M5, sem TASK |
| Catorze módulos de §3.6 sem TASK | não aparecem como opção ativa | TASK própria por módulo |
| Downmix só tem a matriz 6→2 fixada | outros layouts dependem de E01-09 | E01-09 |
