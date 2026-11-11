# Spec — vynamp-engine · pipeline

| Campo | Valor |
|---|---|
| Módulo | `vynamp-engine` |
| Crate(s) | `vynamp-engine` (módulo `pipeline`; crate único até uma TASK justificar divisão) |
| Tipo | lib |
| Versão da spec | 0.2.0 |
| Data | 2026-11-11 |
| Estado | **rascunho autorado, não aprovado** — §3 a §6, §9 e §10 escritos a partir de `reference/audio` e das TASKs E01-05, E01-06, E04-03, E17-07 e E01-10. A aprovação é do mantenedor, por `GOVERNANCE.md` |
| Dono | mantenedor do `vynamp-engine` |
| Depende de | decode, encode, output, dsp (as quatro) |
| É dependido por | player, converter, restore |

> §1, §2 e §7 foram decididos em `01-arvore-final.md` §2 e valem como norma.
> §3 a §6, §9 e §10 foram autorados em 2026-11-11 a partir do material de referência
> de áudio e das TASKs citadas. Estar escrito não é estar aprovado: até o mantenedor
> aprovar, nenhum número aqui é contrato, e toda TASK que dependa dele continua
> `planned`. A máquina de estados e os comandos vivem em `engine.md`; esta spec cobre
> o caminho do sample, da fonte ao dispositivo.

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

### 3.1 Ordem dos estágios

A ordem é fixa e não configurável pelo chamador. Cada stream passa por:

1. fonte → demux → decode (`vynamp-decode`);
2. trim intrínseco (encoder delay/padding) e ganho intrínseco do arquivo;
3. ReplayGain, preamp e reserva de headroom;
4. por stream: EQ (um único modo ativo) → correção/FIR → vitrola → tone, dinâmica e
   efeitos (ver `dsp.md` §3.4 para a cadeia de nós);
5. mixer/crossfade entre streams;
6. volume master `0..=1`;
7. resampler final **único** para a taxa do dispositivo;
8. guard true-peak final em −1 dBTP (E17-07);
9. dither e conversão para o formato do dispositivo;
10. output (`vynamp-output`).

O output **não** aplica ganho, dither nem resample: recebe frames prontos.
O guard true-peak do passo 8 é distinto do limiter criativo da cadeia DSP (E04-07):
o limiter é opcional e fica no passo 4; o guard é sempre o último processamento
antes da quantização e só atua se houver risco de clipping inter-sample.

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Decode, IntrinsicTrim, Gain, StreamDsp, Mixer, Master,
    Resampler, TruePeakGuard, Dither, Output,
}

/// Ordem canônica; testes comparam contra esta constante.
pub const STAGE_ORDER: [Stage; 10] = [
    Stage::Decode, Stage::IntrinsicTrim, Stage::Gain, Stage::StreamDsp, Stage::Mixer,
    Stage::Master, Stage::Resampler, Stage::TruePeakGuard, Stage::Dither, Stage::Output,
];
```

**Modo transparente.** Com `PipelineMode::Transparent`, os passos 3 a 7 e 9 não
existem: nenhum grafo é montado, a taxa do dispositivo é a da fonte e os samples
chegam ao output sem alteração numérica (bit-perfect quando o backend permite,
ver `vynamp-output` `output.md`). O guard do passo 8 também fica desligado: em modo
transparente nada é alterado, por definição.

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineMode { Processed, Transparent }
```

### 3.2 Ring entre decode e callback

Um ring SPSC pré-alocado por stream de saída. O produtor é o worker de decode/DSP;
o consumidor é o callback do output.

```rust
pub struct RingConfig {
    /// Capacidade em frames; alocada uma vez, no `prepare`.
    pub capacity_frames: u32,
    pub channels: u16,
}

pub struct RingStats {
    pub written_frames: u64,   // contador monotônico, u64: não dá a volta em 8 h
    pub read_frames: u64,
    pub underruns: u64,
}
```

Regras:

- contadores `u64`, publicados com `Ordering::Release` e lidos com `Acquire`;
- o callback **só consome**: não aloca, não trava mutex, não faz I/O, não loga;
- underrun: o callback preenche o restante do bloco com zeros e incrementa
  `underruns`; nunca espera;
- EOF: o produtor grava uma marca de fim **depois** do último frame; o consumidor lê a
  marca **antes** de medir o disponível, para não confundir “acabou” com “atrasou”.

### 3.3 Relógio e posição apresentada

A posição que a UI vê é a do sample que **saiu do dispositivo**, não a do que foi
decodificado.

```rust
pub struct PresentedClock { /* seqlock interno */ }

impl PresentedClock {
    /// Frames efetivamente apresentados, monotônico.
    pub fn presented_frames(&self) -> u64;
    pub fn position(&self) -> crate::MediaTime;
}
```

Cálculo, a cada callback:

`presented = max(presented_anterior, frames_consumidos − latency_frames)`

- `latency_frames` vem do output (latência reportada do dispositivo) somada à latência
  declarada dos nós DSP ativos (`dsp.md` §3.1, `latency_frames()`);
- o valor é monotônico dentro de uma geração; seek e `Open` iniciam geração nova e
  zeram a base;
- a leitura é por seqlock: o callback escreve, leitores tentam de novo se pegarem
  escrita no meio; leitor nunca bloqueia o callback;
- erro declarado: subestima a posição real em até `latency_frames` (§10).

### 3.4 Transições entre faixas

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transition {
    /// Padrão. Sem silêncio nem sobreposição entre faixas compatíveis.
    Gapless,
    /// 0 < secs <= 12, curva equal-power. Desligado por padrão.
    Crossfade { secs: f32 },
    /// Para, abre a próxima; pode haver silêncio.
    None,
}
```

- **Gapless** (E01-06): a próxima faixa é aberta e pré-decodificada (2 s) antes do fim
  da atual; trim de encoder delay/padding aplicado nos dois lados. Se taxa ou layout
  diferem e o resampler não cobre a troca sem descontinuidade, o engine emite
  `Event::GaplessDegraded` (ver `engine.md` §3.3) e segue com a troca mais limpa
  possível.
- **Crossfade** (E04-03): equal-power (`cos`/`sin` sobre `π/2`), duração em frames
  calculada na taxa do mixer; 0,5 s a 48 kHz = 24 000 frames.
- Gapless e crossfade se excluem: `Crossfade` ativo desliga a junção gapless.
- Seek durante crossfade cancela o crossfade, descarta a faixa de saída e põe os nós
  com cauda em bypass até o próximo bloco limpo (E04-08).

### 3.5 Threads

| Thread | Faz | Não faz |
|---|---|---|
| control (1) | aplica `Command`, troca snapshots de DSP, publica `Event` | áudio |
| workers de I/O e decode | lê fonte, decodifica, roda a cadeia DSP do stream | toca o callback |
| callback do output | consome o ring, aplica master, guard, dither | aloca, trava, faz I/O |
| leitores (UI) | leem `Snapshot` e `PresentedClock` | escrevem estado |

## 4. Requisitos

| ID | Requisito | REQ de produto | TASK | Onde é provado |
|---|---|---|---|---|
| PIPE-REQ-01 | A ordem dos estágios é exatamente `STAGE_ORDER`; nenhum chamador reordena — planned (M1) | ordem canônica do sinal | E01-05 | — (PIPE-AC-01 planejado) |
| PIPE-REQ-02 | Em `Transparent`, a saída é igual à entrada sample a sample quando taxa e formato coincidem — planned (M1) | reprodução bit-perfect | E01-05 | — (PIPE-AC-02 planejado) |
| PIPE-REQ-03 | O callback não aloca, não trava mutex e não faz I/O — planned (M1) | áudio sem falhas | E01-05 | — (PIPE-AC-03 planejado) |
| PIPE-REQ-04 | Underrun gera zeros e incrementa o contador; nunca bloqueia — planned (M1) | áudio sem falhas | E01-05 | — (PIPE-AC-04 planejado) |
| PIPE-REQ-05 | EOF drena até o último frame; nada da faixa é cortado nem repetido — planned (M1) | reprodução completa | E01-05 | — (PIPE-AC-05 planejado) |
| PIPE-REQ-06 | A posição apresentada é monotônica por geração e nunca passa do áudio que saiu — planned (M1) | posição confiável | E01-05 | — (PIPE-AC-06 planejado) |
| PIPE-REQ-07 | Gapless: junção sem silêncio nem clique entre faixas com mesma taxa e layout, com trim aplicado — planned (M2) | gapless | E01-06 | — (PIPE-AC-07 planejado) |
| PIPE-REQ-08 | Crossfade equal-power com duração exata em frames; mutuamente exclusivo com gapless — planned (M2) | crossfade | E04-03 | — (PIPE-AC-08 planejado) |
| PIPE-REQ-09 | Guard true-peak final mantém a saída ≤ −1 dBTP e fica depois do resampler, antes da quantização — planned (M3) | sem clipping | E17-07 | — (PIPE-AC-09 planejado) |
| PIPE-REQ-10 | Existe um único resampler no caminho processado — planned (M1) | qualidade de conversão | E01-05 | — (PIPE-AC-10 planejado) |
| PIPE-REQ-11 | Callback p99,9 abaixo de 25% do período do bloco e 8 h sem underrun no perfil de referência — planned (M3) | estabilidade | E01-10 | — (PIPE-AC-11 planejado) |

Os rótulos da coluna “REQ de produto” são descritivos; o ID de produto correspondente
é o da TASK citada no backlog.

## 5. Critérios de aceite

| ID | Given | When | Then | Teste |
|---|---|---|---|---|
| `PIPE-AC-01` | um pipeline em `Processed` | inspeciona-se a cadeia montada | a ordem é igual a `STAGE_ORDER` | `stage_order_is_canonical` |
| `PIPE-AC-02` | fonte PCM 44,1 kHz s16 e dispositivo nulo 44,1 kHz s16 em `Transparent` | toca-se a faixa inteira | os bytes capturados são idênticos aos decodificados | `transparent_is_bit_exact` |
| `PIPE-AC-03` | alocador de teste que falha ao alocar na thread do callback | toca-se 10 s | nenhuma alocação é registrada no callback | `callback_never_allocates` |
| `PIPE-AC-04` | produtor pausado artificialmente | o callback roda 3 blocos | saem zeros, `underruns` = 3, o callback não espera | `underrun_fills_zeros` |
| `PIPE-AC-05` | fixture de duração conhecida em frames | toca-se até o fim | frames capturados = frames da fixture após trim, último frame igual | `eof_emits_every_frame` |
| `PIPE-AC-06` | latência simulada de 1024 frames | lê-se a posição a cada callback | a série é não decrescente e nunca excede `consumidos − 1024` | `presented_clock_is_monotonic` |
| `PIPE-AC-07` | par de fixtures gapless com delay/padding conhecidos | toca-se a sequência | a junção concatenada é igual à referência sem amostras extras | `gapless_join_matches_reference` |
| `PIPE-AC-08` | duas faixas a 48 kHz e crossfade de 0,5 s | ocorre a transição | a região dura 24 000 frames e a soma de potências fica em 1 ± 0,01 | `crossfade_is_equal_power` |
| `PIPE-AC-09` | sinal com picos inter-sample acima de 0 dBTP | passa pelo guard | o true-peak medido (oversampling 4×) é ≤ −1 dBTP | `true_peak_guard_holds` |
| `PIPE-AC-10` | fonte 44,1 kHz e dispositivo 48 kHz com DSP ativo | monta-se a cadeia | há exatamente um estágio `Resampler` | `single_resampler` |
| `PIPE-AC-11` | perfil de referência, `VYNAMP_SOAK_FIXTURE` definido | toca-se 8 h | zero underruns e callback p99,9 < 25% do período | `soak_8h` (ignore) |

**Nenhum destes testes existe ainda.** O nome é o compromisso; a TASK citada em §4 é
quem os cria.

### Critérios que esta spec **não** fixa

- Algoritmo e qualidade do resampler (filtro, fase) — decisão da TASK E01-05.
- Tipo de dither e noise shaping — decisão de E01-05, com default declarado lá.
- Tamanho padrão do ring em frames — medido na TASK contra as metas de latência de
  `engine.md` §4.

## 6. Erros e cancelamento

**Erro é dado, não `panic!`.** O pipeline devolve `crate::Error` (definido em
`engine.md` §6); não define enum próprio.

| Situação | Variante | O que o chamador faz |
|---|---|---|
| decode falha no meio da faixa | `Error::Decode` | o engine encerra a faixa, emite `Event::Error` e segue para a próxima da fila |
| output rejeita formato | `Error::Output` | o engine tenta o próximo formato suportado; sem nenhum, vai a `Error` |
| nó DSP falha no `prepare` | `Error::Dsp` | o nó entra em bypass, ver `dsp.md` §6; a reprodução continua |
| dispositivo some | `Error::DeviceLost` | estado `DeviceLost`, ver `engine.md` §3.2 |

NaN ou ±Inf na saída de um nó põem **esse nó** em bypass com crossfade curto e
geram `Event::Error` uma vez por nó por geração; o resto da cadeia segue.

**Cancelamento é explícito, não timeout.** Seek, Stop e Open incrementam a geração
(`engine.md` §3.4); o worker confere `CancelToken` entre blocos e descarta o que for
de geração antiga. O ring é esvaziado pelo consumidor quando a marca de geração muda,
para que nenhum frame velho seja tocado.

## 7. Fora de escopo

Vinyl/Tube e VST3 entram como modulos internos deste grafo (E19-07, E12-07). Jobs persistidos (E08-01) entram como infraestrutura nao-audio, decisao registrada

## 8. Dependências

Depende de: decode, encode, output, dsp (as quatro)

É dependido por: player, converter, restore

## 9. Testes e fixtures

- Fixtures de áudio: as de `libs/decode/tests/fixtures/`, com o `MANIFEST` como
  fonte de duração, taxa e frames esperados. Pares gapless entram ali pela E01-06.
- Dispositivo: `NullHost` do `vynamp-output`, que captura os frames entregues e
  permite simular latência e underrun.
- Oráculos numéricos:
  - bit-exatidão em `Transparent` por comparação byte a byte;
  - potência do crossfade: `g_out² + g_in² = 1 ± 0,01` em cada frame;
  - true-peak por oversampling 4× (mesmo método do medidor de E17-07).
- Testes longos: `soak_8h` fica atrás de
  `#[ignore = "8 h de reprodução; defina VYNAMP_SOAK_FIXTURE"]` e lê o caminho da
  fixture longa dessa variável.

## 10. Limitações conhecidas

Limitação declarada não reprova a entrega; limitação omitida reprova.

| Limitação | Efeito | Quem cobre |
|---|---|---|
| a posição apresentada subestima a real em até `latency_frames` | barra de progresso até ~20 ms atrás em dispositivos típicos | aceito; documentado na UI |
| gapless entre faixas com taxa ou layout diferentes pode degradar | `Event::GaplessDegraded` e possível clique curto | E01-06 |
| modo transparente desliga EQ, volume e guard | o volume tem de ser feito no dispositivo/sistema | aceito por definição |
| a latência reportada pelo backend pode estar errada | posição e A/V desalinham pelo erro do driver | `vynamp-output` `backends.md` |
