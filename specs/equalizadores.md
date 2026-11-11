# Spec — vynamp-engine · equalizadores

| Campo | Valor |
|---|---|
| Módulo | `vynamp-engine` |
| Crate(s) | `vynamp-engine` (módulo `eq`; crate único até uma TASK justificar divisão) |
| Tipo | lib |
| Versão da spec | 0.2.0 |
| Data | 2026-11-11 |
| Estado | **rascunho autorado, não aprovado** — §3 a §6, §9 e §10 escritos a partir de `reference/audio/08-equalizers.md` e das TASKs E04-01, E04-04 e E12-10. A aprovação é do mantenedor, por `GOVERNANCE.md` |
| Dono | mantenedor do `vynamp-engine` |
| Depende de | decode, encode, output, dsp (as quatro) |
| É dependido por | player, converter, restore |

> §1, §2 e §7 foram decididos em `01-arvore-final.md` §2 e valem como norma. §3 a §6,
> §9 e §10 foram autorados em 2026-11-11 a partir do material de referência de áudio e
> das TASKs citadas. Estar escrito não é estar aprovado: até o mantenedor aprovar,
> nenhum número aqui é contrato, e toda TASK que dependa dele continua `planned`.
> Esta spec fixa o EQ gráfico (10, 20, 31 e 64 bandas), o EQ paramétrico (PEQ), o
> preamp, o volume/pan e o ReplayGain. O contrato de nó, o bypass, o smoothing, a
> ordem de slots e os presets são de `dsp.md`; aqui só se diz o que cada nó calcula.

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

### 3.1 Modos e grades

```rust
pub enum EqMode { Graphic10, Graphic20, Graphic31, Graphic64, Parametric }

/// Centros exatos, em Hz, calculados pela fórmula da tabela abaixo.
pub fn band_centers(mode: EqMode) -> &'static [f64];
/// Q constante do modo gráfico: Q = √(2^N) / (2^N − 1), N = largura em oitavas.
pub fn band_q(mode: EqMode) -> f64;
```

| Modo | Bandas | Centro `f_k` (Hz) | Faixa | Q | Marco |
|---|---|---|---|---|---|
| `Graphic10` | 10 | `1000·2^(k−5)`, k = 0..9 | 31,25 Hz a 16 kHz | ≈ 1,41 (1 oitava) | M1 |
| `Graphic20` | 20 | `1000·2^((k−11)/2)`, k = 0..19 | 22,1 Hz a 16 kHz | ≈ 2,87 (½ oitava) | M3 |
| `Graphic31` | 31 | `1000·10^(k/10)`, k = −17..13 | 19,95 Hz a 19,95 kHz (rótulos ISO 266) | ≈ 4,32 (⅓ oitava) | M3 |
| `Graphic64` | 64 | `20·1000^(k/63)`, k = 0..63 | 20 Hz a 20 kHz (razão 1,11588) | ≈ 9,12 | M5 |
| `Parametric` | ≤ 32 filtros (M3), ≤ 64 (M5) | livre, ver §3.3 | 10 Hz ao limite de Nyquist | 0,1 a 18 | M3 / M5 |

Os rótulos exibidos do EQ-31 são os nominais da ISO 266 (20, 25, 31,5 … 20k); o filtro
usa o centro exato. O EQ-64 **não** é ⅓ de oitava e não reusa rótulos ISO.

Cada banda gráfica é um biquad *peaking* RBJ de Q constante; as bandas ficam em cascata
depois do preamp. Coeficientes e estado são `f64`.

### 3.2 Estado

```rust
pub struct EqState {
    pub active_mode: EqMode,
    pub enabled: bool,
    pub graphic10: GraphicBands<10>,
    pub graphic20: GraphicBands<20>,
    pub graphic31: GraphicBands<31>,
    pub graphic64: GraphicBands<64>,
    pub parametric: ParametricState,
}

pub struct GraphicBands<const N: usize> { pub preamp_db: f64, pub gains_db: [f64; N] }

pub struct ParametricState { pub preamp_db: f64, pub filters: Vec<PeqFilter> }
```

Os modos são independentes: cada um guarda o próprio preamp e os próprios ganhos.
Trocar de modo não converte valores; uma conversão só existe como comando explícito do
usuário, fora deste contrato.

| Grandeza | Faixa |
|---|---|
| preamp (todo modo) | −24 a +12 dB |
| ganho de banda, painel clássico | ±12 dB |
| ganho de banda, editor avançado | ±15 dB |
| ganho de filtro PEQ | −24 a +24 dB |

O painel clássico e o editor avançado escrevem no mesmo `gains_db`; o painel só limita
a faixa de edição. Um valor fora de ±12 dB vindo do editor é mostrado no painel no
limite, sem ser alterado.

### 3.3 Filtros paramétricos

```rust
pub struct PeqFilter {
    pub id: FilterId,
    pub enabled: bool,
    pub kind: FilterKind,
    pub frequency_hz: f64,
    pub gain_db: Option<f64>,     // só Peaking, LowShelf, HighShelf
    pub q: f64,
    pub slope_db_oct: Option<u8>, // só LowPass, HighPass
    pub channel: Channel,
}

pub enum FilterKind { Peaking, LowShelf, HighShelf, LowPass, HighPass, Notch, BandPass }
pub enum Channel { Both, Left, Right }
```

| Campo | Faixa | Observação |
|---|---|---|
| `frequency_hz` | 10 Hz a `nyquist_limit(fs)` | §3.4 |
| `q` | 0,1 a 18 | — |
| `gain_db` | −24 a +24 dB | ausente nos tipos sem ganho |
| `slope_db_oct` | 12 | 24, 36 e 48 dB/oct são declarados em §10 como "a validar" |
| filtros | ≤ 32 em M3; ≤ 64 em M5 | o 33º (ou 65º) é recusado com motivo |

### 3.4 Nyquist e disponibilidade

```rust
/// min(40 000, 0,45 · fs)
pub fn nyquist_limit(fs: u32) -> f64;

pub enum Availability { Available, Unavailable { limit_hz: f64 } }
pub fn band_availability(mode: EqMode, fs: u32) -> Vec<Availability>;
```

Uma banda ou filtro cujo centro passa do limite fica `Unavailable`: o valor guardado é
preservado, o filtro entra em bypass e o motivo é exibido. **Mover a banda em silêncio é
proibido.** Quando a taxa volta a permitir, a banda volta com o valor guardado.

| Fs | Limite | EQ-10 indisponível | EQ-20 indisponível | EQ-31 indisponível | EQ-64 indisponível |
|---|---|---|---|---|---|
| 32 kHz | 14,4 kHz | 16k | 16k | 16k, 20k | bandas 62–64 |
| 44,1 kHz | 19,845 kHz | — | — | 20k | banda 64 |
| ≥ 48 kHz | ≥ 21,6 kHz | — | — | — | — |

O EQ roda na taxa do stream, antes do resampler (`pipeline.md`, estágio `StreamDsp`).
Mudança de `fs` recalcula coeficientes fora do callback e troca a cadeia com crossfade
(`dsp.md` §3.4).

### 3.5 Curva calculada e headroom

```rust
/// H(f) = G_preamp · ∏ H_k(f), em dB, em `points` pontos logarítmicos
/// de 10 Hz a min(40 kHz, fs/2). Bandas `Unavailable` entram como bypass.
pub fn response(state: &EqState, fs: u32, points: usize) -> Vec<(f64, f64)>;

/// −max(0, pico_dB da curva). Só é aplicado por ação do usuário.
pub fn headroom_suggestion(state: &EqState, fs: u32) -> f64;
```

`points` mínimo é 512. A curva é a mesma que o áudio recebe: é calculada a partir dos
mesmos coeficientes que o nó usa, não de uma aproximação de UI.

### 3.6 Nós e slots

| Nó | Slot (`dsp.md` §3.4) | O que faz | TASK |
|---|---|---|---|
| `Preamp` | `Preamp` | ganho do modo ativo e ReplayGain (track/album, com prevenção de clip) | E04-01 |
| `Equalizer` | `Eq` | cascata gráfica ou PEQ do modo ativo | E04-01, E04-04, E12-10 |
| `VolumePan` | `VolumePan` | volume e pan equal-power | E04-01 |

Desligar o EQ (`enabled = false`) é bypass real: a saída é a entrada, amostra por
amostra. Ligar o EQ com o caminho em modo transparente pede confirmação ao player, e
ligar tira o caminho do modo transparente (`pipeline.md`). Os biquads têm latência zero.

### 3.7 Presets

Os namespaces de preset são `eq10`, `eq20`, `eq31`, `eq64` e `peq`. O formato é
`schemas/audio-preset.schema.json` v2, com migração v1→v2, entregue por E04-04; escopo,
precedência e recusa de preset inválido são de `dsp.md` §3.5. Um preset de um modo
nunca escreve em outro modo.

## 4. Requisitos

| ID | Requisito | REQ de produto | TASK | Onde é provado |
|---|---|---|---|---|
| `EQ-REQ-01` | O EQ-10 usa os centros e o Q de §3.1, preamp −24 a +12 dB por modo, e ganho ±12 dB no painel clássico — planned (M1) | `REQ-E04-01` | `TASK-E04-01` | — (EQ-AC-01 planejado) |
| `EQ-REQ-02` | A resposta de cada modo gráfico fica dentro de ±0,2 dB da cascata RBJ calculada fora do código sob teste, a 44,1, 48, 96 e 192 kHz, sem `NaN` — planned (M1 para EQ-10, M3 para EQ-20/31) | `REQ-E04-01`, `REQ-E04-04` | `TASK-E04-01`, `TASK-E04-04` | — (EQ-AC-02 planejado) |
| `EQ-REQ-03` | `enabled = false` é bypass real, amostra por amostra — planned (M1) | `REQ-E04-01` | `TASK-E04-01` | — (EQ-AC-03 planejado) |
| `EQ-REQ-04` | Volume/pan equal-power e ReplayGain track/album com prevenção de clip no `Preamp` — planned (M1) | `REQ-E04-01` | `TASK-E04-01` | — (EQ-AC-04 planejado) |
| `EQ-REQ-05` | Os modos guardam estado independente; trocar de modo não perde nem converte valor — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-05 planejado) |
| `EQ-REQ-06` | O PEQ aceita os 7 tipos de §3.3, com as faixas de §3.3, até 32 filtros; o 33º é recusado com motivo — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-06 planejado) |
| `EQ-REQ-07` | Banda ou filtro acima de `nyquist_limit(fs)` fica `Unavailable` com motivo, valor preservado e bypass; nunca é movido — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-07 planejado) |
| `EQ-REQ-08` | Valor fora de faixa, `NaN` ou texto inválido é recusado e o valor anterior fica; import inválido mantém o preset anterior e aponta o erro por JSON pointer — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-08 planejado) |
| `EQ-REQ-09` | `response` coincide com a cascata aplicada ao áudio dentro de ±0,1 dB, com ≥ 512 pontos — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-09 planejado) |
| `EQ-REQ-10` | Um filtro com polo fora do círculo unitário é recusado antes da troca; a cadeia vigente continua — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-10 planejado) |
| `EQ-REQ-11` | Ganho positivo com pico acima de 0 dB gera aviso de headroom; a sugestão só é aplicada por ação do usuário — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-11 planejado) |
| `EQ-REQ-12` | O preset v2 e a migração v1→v2 seguem `audio-preset.schema.json`; um preset de um namespace não escreve em outro modo — planned (M3) | `REQ-E04-04` | `TASK-E04-04` | — (EQ-AC-12 planejado) |
| `EQ-REQ-13` | O EQ-64 e o PEQ de 64 filtros rodam por canal em `f64` de 44,1 a 384 kHz; até M5 aparecem como "indisponível nesta versão (M5)" — planned (M5) | `REQ-E12-10` | `TASK-E12-10` | — (EQ-AC-13 planejado) |
| `EQ-REQ-14` | O custo de CPU do EQ-64 é medido em benchmark de 44,1 a 384 kHz, não estimado — planned (M5) | `REQ-E12-10` | `TASK-E12-10` | — (EQ-AC-14 planejado) |

Os nomes de teste em §5 são rótulos descritivos; o nome final é fixado pela TASK e
registrado na coluna "Onde é provado".

## 5. Critérios de aceite

| ID | Given | When | Then | Teste |
|---|---|---|---|---|
| `EQ-AC-01` | os quatro modos gráficos | `band_centers` e `band_q` são lidos | todo centro tem erro ≤ 0,1 % contra a fórmula de §3.1 e todo Q coincide com `√(2^N)/(2^N−1)` | `band_grids_match_formula` |
| `EQ-AC-02` | EQ-10 (M1) e EQ-20/31 (M3) com todas as bandas em +6 dB, depois em −15 dB, a 44 100, 48 000, 96 000 e 192 000 Hz | a resposta é medida por impulso e comparada ponto a ponto, de 10 Hz a min(40 kHz, Fs/2), com a cascata RBJ calculada fora do código sob teste | desvio ≤ ±0,2 dB e nenhum `NaN` | `graphic_response_matches_rbj` |
| `EQ-AC-03` | `sine-f32le.wav` passando pelo EQ-10 com ganhos não nulos | `enabled` vai a `false` | a saída é idêntica à entrada, amostra por amostra | `disabled_eq_is_sample_exact` |
| `EQ-AC-04` | faixa com ReplayGain track de +8 dB e pico de −2 dBFS, e pan em −1, 0 e +1 | a faixa toca com ReplayGain ligado e prevenção de clip | o ganho aplicado é limitado para o pico não passar de 0 dBFS, e a soma de potência L+R é constante nas três posições de pan | `replaygain_and_pan_are_bounded` |
| `EQ-AC-05` | os cinco modos com valores distintos | o modo ativo troca 20 vezes, em ordem aleatória com semente fixa | nenhum valor de nenhum modo muda | `mode_switches_lose_nothing` |
| `EQ-AC-06` | um PEQ de 32 filtros — 8 peaking, 4 low shelf, 4 high shelf, 4 low pass, 4 high pass, 4 notch e 4 band pass —, de 20 Hz a 18 kHz, Q 0,7 a 8, ganho ±6 dB onde há ganho, a 44,1/48/96/192 kHz | a resposta é medida e um 33º filtro é adicionado | a resposta fica em ±0,2 dB da cascata de referência, e o 33º é recusado com motivo, sem alterar os 32 | `peq32_response_and_cap` |
| `EQ-AC-07` | EQ-31 com a banda de 20 kHz em +6 dB e a de 16 kHz ativa, e a grade do EQ-64 como fixture de disponibilidade, a 44 100 Hz | a taxa vai a 32 000, depois 48 000, depois 44 100 Hz | em cada taxa a disponibilidade bate com a tabela de §3.4, o valor guardado não muda, o motivo é exibido e a curva trata a banda como bypass | `nyquist_table_is_respected` |
| `EQ-AC-08` | um preset `peq` válido com 32 filtros e preamp −3 dB, serializado como referência, e quatro imports: `q` 0,05, `gain_db` 40, `frequency_hz` 45 000 e `q` como texto `NaN`; além de `set` direto com Q 19 e frequência 5 Hz | cada import e cada `set` é aplicado em sequência | todos são recusados, o estado e a serialização ficam idênticos à referência, e cada erro traz código e JSON pointer | `invalid_values_keep_previous` |
| `EQ-AC-09` | 50 estados aleatórios de semente fixa, cobrindo os cinco modos | `response(state, 48 000, 512)` é comparada com a resposta medida do nó | desvio ≤ ±0,1 dB em todos os pontos | `curve_matches_cascade` |
| `EQ-AC-10` | um filtro de teste cujos coeficientes calculados têm polo fora do círculo unitário | o filtro é aplicado | é recusado antes da troca e a cadeia vigente segue tocando sem descontinuidade | `unstable_filter_is_refused` |
| `EQ-AC-11` | EQ-10 com +9 dB em 63 Hz e preamp 0 dB, e o limiter de `catalogo-dsp.md` com teto −1 dBTP | a curva é calculada e a faixa toca | o aviso de headroom aparece com a sugestão `−máx(0, pico_dB)`, o preamp não muda sozinho, e o pico final fica ≤ teto + 0,1 dBTP | `headroom_warning_is_advisory` |
| `EQ-AC-12` | um preset v1 de `eq10` e um preset v2 de `eq31` | os dois são importados com o EQ-20 ativo | o v1 migra para v2 sem perda, cada um escreve só no próprio modo e o EQ-20 fica intacto | `presets_migrate_and_stay_in_namespace` |
| `EQ-AC-13` | EQ-64 e PEQ de 64 filtros por canal, a 44 100, 96 000, 192 000 e 384 000 Hz | a resposta é medida em L e R com curvas diferentes | cada canal fica em ±0,2 dB da própria cascata de referência; num build anterior a M5 o modo aparece como "indisponível nesta versão (M5)" | `eq64_per_channel_f64` |
| `EQ-AC-14` | EQ-64 com as 64 bandas ativas | um benchmark roda blocos de 512 frames estéreo de 44,1 a 384 kHz | o tempo por bloco é registrado por taxa num relatório versionado | `eq64_cpu_benchmark` (ignore) |

**Nenhum destes testes existe ainda.** Os valores numéricos vêm de
`reference/audio/08-equalizers.md` (EQ-T-01..15) e dos AC de `TASK-E04-04`.

### Critérios que esta spec **não** fixa

- A ausência de clique na troca de valor e de modo: o smoothing e o crossfade são de
  `dsp.md` (NODE-AC-02 e NODE-AC-06), e o limiar de descontinuidade é de E10-03.
- O teto de CPU do EQ-64: só existe depois do benchmark de EQ-AC-14 (E12-10).
- O editor visual, a arrastar bandas e a exibir a curva: é da UI (E02-07).
- Os mapas de canal além de estéreo: são de E01-09.
- O removedor de voz: é de `catalogo-dsp.md` (E04-08).

## 6. Erros e cancelamento

**Erro é dado, não `panic!`.** Falhas do EQ usam um tipo próprio, convertido em
`NodeError` (`dsp.md` §6) quando atravessa o grafo:

```rust
pub enum EqError {
    OutOfRange { field: String, value: f64, min: f64, max: f64 },
    NonFinite { field: String },
    FilterLimit { max: usize },
    UnstableFilter { id: FilterId },
    ModeUnavailable { mode: EqMode, since: &'static str },
    PresetInvalid { pointer: String, reason: String },
}
```

| Situação | Efeito |
|---|---|
| valor fora de faixa ou não finito | recusado; o valor anterior fica e o erro nomeia o campo |
| 33º filtro (ou 65º em M5) | recusado com `FilterLimit` |
| filtro instável | recusado antes da troca com `UnstableFilter`; a cadeia vigente continua |
| banda acima de Nyquist | **não é erro**: vira `Availability::Unavailable` com motivo |
| modo M5 num build anterior | `ModeUnavailable { since: "M5" }` |
| import inválido | `PresetInvalid` com JSON pointer; o preset anterior fica |

Nenhuma dessas situações para a reprodução.

**Cancelamento é explícito, não timeout.** O recálculo de coeficientes por mudança de
taxa ou de estado roda fora do callback e é descartado quando uma geração mais nova
chega (`dsp.md` §6). O EQ não tem prazo próprio.

## 7. Fora de escopo

Vinyl/Tube e VST3 entram como modulos internos deste grafo (E19-07, E12-07). Jobs persistidos (E08-01) entram como infraestrutura nao-audio, decisao registrada

## 8. Dependências

Depende de: decode, encode, output, dsp (as quatro)

É dependido por: player, converter, restore

## 9. Testes e fixtures

| Fixture | Origem | Uso |
|---|---|---|
| `tests/fixtures/graph/sine-f32le.wav` e demais PCM | cópia de `libs/decode/tests/fixtures/pcm`, SHA-256 conferido contra `MANIFEST.txt` | EQ-AC-03, EQ-AC-04 |
| `tests/fixtures/eq/presets/` | escrito pela TASK: presets v1 e v2 válidos e os quatro imports inválidos de EQ-AC-08 | EQ-AC-08, EQ-AC-12 |
| referência RBJ | calculada no próprio teste a partir das fórmulas do Audio EQ Cookbook, sem usar o código sob teste | EQ-AC-02, EQ-AC-06, EQ-AC-09, EQ-AC-13 |

O oráculo é numérico: impulso unitário pela cadeia, FFT da resposta e comparação ponto
a ponto com a cascata calculada. O benchmark de EQ-AC-14 vive atrás de
`#[ignore = "benchmark de CPU; roda com VYNAMP_BENCH=1"]`.

```bash
cd libs/engine && cargo test --test eq --locked
```

## 10. Limitações conhecidas

Limitação declarada não reprova a entrega; limitação omitida reprova.

| Limitação | Efeito | Quem cobre |
|---|---|---|
| Inclinações de 24, 36 e 48 dB/oct no PEQ estão "a validar" | só 12 dB/oct é contrato; as outras não aparecem como opção até serem validadas | E04-04 |
| Interação entre bandas vizinhas de Q alto não é compensada | a curva mostra a soma real, que pode passar do ganho de cada banda | investigação aberta; nenhuma TASK ainda |
| Ripple entre centros do EQ-64 (Q ≈ 9,12) | uma curva plana a +X dB em todas as bandas não é exatamente +X dB entre centros | E12-10 mede e declara |
| Editor por canal e multicanal | antes de M5 o PEQ só aplica `Both`, `Left` e `Right` em estéreo | E12-10, E01-09 |
| CPU do EQ-64 | sem número até o benchmark; pode exigir desligar em taxas altas | E12-10 |
