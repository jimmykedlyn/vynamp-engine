# Spec — vynamp-engine · dsp

| Campo | Valor |
|---|---|
| Módulo | `vynamp-engine` |
| Crate(s) | `vynamp-engine` (módulo `graph`; crate único até uma TASK justificar divisão) |
| Tipo | lib |
| Versão da spec | 0.2.0 |
| Data | 2026-11-11 |
| Estado | **rascunho autorado, não aprovado** — §3 a §6, §9 e §10 escritos a partir de `reference/audio/03-dsp.md`, `reference/audio/10-vitrola.md` e das TASKs E04-03, E04-01, E19-03, E19-07 e E12-07. A aprovação é do mantenedor, por `GOVERNANCE.md` |
| Dono | mantenedor do `vynamp-engine` |
| Depende de | decode, encode, output, dsp (as quatro) |
| É dependido por | player, converter, restore |

> §1, §2 e §7 foram decididos em `01-arvore-final.md` §2 e valem como norma.
> §3 a §6, §9 e §10 foram autorados em 2026-11-11 a partir do material de referência
> de áudio e das TASKs citadas. Estar escrito não é estar aprovado: até o mantenedor
> aprovar, nenhum número aqui é contrato, e toda TASK que dependa dele continua
> `planned`. Esta spec cobre o **grafo DSP do engine**: o contrato de nó, a troca de
> parâmetros, o bypass, a cadeia por stream e os presets. As primitivas (resampler,
> dither, ganho, fade, mixer) são da lib `vynamp-dsp` (`libs/dsp/specs/dsp.md`) e não
> são redefinidas aqui. Os equalizadores estão em `equalizadores.md` e os demais nós
> em `catalogo-dsp.md`.

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

### 3.1 Contrato de nó

Todo processador do grafo — EQ, vitrola, dinâmica, efeito, plugin isolado — implementa
o mesmo trait. Ele espelha o `Processor` do vitrola para que o adaptador (§3.6) seja
fino.

```rust
pub trait Node: Send {
    /// Metadados estáveis: ID, versão, parâmetros, custo e canais suportados.
    fn descriptor(&self) -> &NodeDescriptor;

    /// Aloca buffers e calcula estado. Roda FORA da thread de áudio.
    fn prepare(&mut self, sample_rate: u32, max_block_frames: u32,
               layout: ChannelLayout) -> Result<(), NodeError>;

    /// Processa um bloco no lugar. Roda NA thread de áudio: não aloca,
    /// não trava, não faz I/O, tempo limitado.
    fn process(&mut self, block: &mut AudioBlock<'_>, ctx: &BlockContext);

    /// Zera o estado interno (caudas, linhas de atraso). Fora da RT.
    fn reset(&mut self);

    /// Atraso que o nó introduz, em frames na taxa preparada.
    fn latency_frames(&self) -> u32;

    /// Frames de cauda após silêncio na entrada (reverb, delay).
    fn tail_frames(&self) -> u32;
}

pub struct NodeDescriptor {
    pub id: NodeId,              // estável entre versões, ex.: "eq.graphic10"
    pub version: u32,            // muda quando o estado de preset muda de forma
    pub params: &'static [ParamSpec],
    pub cost: CostClass,         // Light | Medium | Heavy
    pub channels: ChannelSupport,
}

pub struct ParamSpec {
    pub id: &'static str,
    pub unit: Unit,              // Db | Hz | Q | Ratio | Ms | Percent | Bool | Enum
    pub min: f64, pub max: f64, pub default: f64,
    pub smoothable: bool,
}

pub struct AudioBlock<'a> { /* planar f32; frames <= max_block_frames */ }

pub struct BlockContext {
    pub position_frames: u64,
    pub generation: u64,         // muda a cada seek ou troca de faixa
    pub transport: Option<TransportEvent>,
    pub track: Option<TrackMeta>,
}

pub enum TransportEvent { Start, Stop, Pause, Seek, TrackChanged, AlbumEnded }
```

`AudioBlock` é planar em `f32` dentro do grafo. Nós que precisam de `f64` internamente
(E12-10) convertem dentro do próprio `process`; o grafo não carrega dois formatos.

### 3.2 Snapshot de parâmetros

```rust
pub struct ParamSnapshot { /* imutável; coeficientes já calculados */ }

impl GraphHandle {
    /// Valida, calcula coeficientes na control thread e publica.
    pub fn set_params(&self, node: NodeId, values: &[(&str, f64)])
        -> Result<(), NodeError>;
}
```

A UI gera valores; a control thread valida contra `ParamSpec`, calcula coeficientes e
publica um `ParamSnapshot` novo por ponteiro atômico. O callback troca de snapshot na
fronteira do bloco e interpola os parâmetros `smoothable` por 10 a 50 ms. O snapshot
antigo é devolvido por fila e descartado fora da RT.

### 3.3 Bypass e smoothing

```rust
impl GraphHandle {
    pub fn set_bypass(&self, node: NodeId, bypass: bool);
    pub fn set_global_bypass(&self, bypass: bool);
    pub fn node_status(&self, node: NodeId) -> NodeStatus;
}

pub struct NodeStatus { pub state: NodeState, pub reason: Option<BypassReason> }
pub enum NodeState { Active, Bypassed, Unavailable }
pub enum BypassReason {
    User, PrepareFailed, NonFinite, Unstable, PluginHung,
    TransparentMode, DsdPath, UnsupportedRate, UnsupportedLayout,
}
```

Ligar ou desligar bypass, por nó ou global, é sempre com crossfade equal-power. A
duração do crossfade de bypass é a mesma do smoothing (10 a 50 ms, fixada na TASK
E04-03). Bypass de um nó com latência mantém a latência anunciada, para a posição
apresentada (`pipeline.md` §3.3) não saltar.

### 3.4 Cadeia por stream

O grafo **não** é livre. D-DSP-10 e D-DSP-12 continuam abertos; até serem decididos,
o grafo é uma cadeia compilada de ordem fixa por stream, entre os estágios `StreamDsp`
e `Master` de `pipeline.md` §3.1:

```rust
pub enum Slot {
    Preamp,          // ganho de entrada e ReplayGain (E04-01)
    Eq,              // um EQ ativo por vez: Graphic10/20/31, Peq32, Eq64 (equalizadores.md)
    Correction,      // correção de fone/caixa (AutoEQ, quando existir)
    Vitrola,         // vinil e válvula (E19-07)
    Dynamics,        // compressor e limiter criativo (catalogo-dsp.md)
    Spatial,         // imagem, downmix, voice remover
    Creative,        // tempo/pitch, reverb, delay, modulação (catalogo-dsp.md)
    Plugin,          // VST3 isolado (E12-07)
    VolumePan,       // volume e pan finais do stream (E04-01)
}

pub const SLOT_ORDER: [Slot; 9] = [
    Slot::Preamp, Slot::Eq, Slot::Correction, Slot::Vitrola, Slot::Dynamics,
    Slot::Spatial, Slot::Creative, Slot::Plugin, Slot::VolumePan,
];

pub struct GraphSpec { /* nós por slot; validado por compile() */ }

impl GraphSpec {
    pub fn compile(&self, sample_rate: u32, max_block_frames: u32,
                   layout: ChannelLayout) -> Result<CompiledGraph, NodeError>;
}

impl CompiledGraph {
    pub fn latency_frames(&self) -> u32; // soma das latências dos nós ativos
}
```

`compile` roda fora da RT, chama `prepare` em cada nó e devolve uma cadeia pronta. A
troca de cadeia (troca de modo de EQ, nó adicionado ou removido) é feita por geração:
a cadeia nova é compilada fora da RT, publicada por ponteiro atômico e entra com
crossfade equal-power entre a cadeia antiga e a nova. Dentro de um slot a ordem dos nós
é declarada; entre slots, é a de `SLOT_ORDER`. Nada pode ser colocado depois do
`TruePeakGuard` (`pipeline.md` §3.1).

O vitrola senta depois de EQ e correção e antes de dinâmica, imagem e criativos,
conforme `reference/audio/10-vitrola.md`.

### 3.5 Presets

```rust
pub struct Preset {
    pub schema_version: u32,
    pub uuid: Uuid,
    pub name: String,
    pub author: Option<String>,
    pub nodes: Vec<NodeState>,       // por slot: NodeId, versão, parâmetros
    pub checksum: [u8; 32],          // SHA-256 do conteúdo canônico
    pub provenance: Option<String>,
}

pub enum PresetScope { Session, Track, Genre, Device, Global }

impl GraphHandle {
    pub fn apply_preset(&self, preset: &Preset) -> Result<PresetReport, NodeError>;
}

pub struct PresetReport { pub adapted: Vec<(NodeId, String)> }
```

Preset é JSON versionado. Parâmetros são independentes da taxa: frequências em Hz,
tempos em ms; os coeficientes saem em `prepare`. A precedência de escopo é
`Session > Track > Genre > Device > Global`. Os presets de uso de fábrica são
Original/Neutro, Fones e Caixas estéreo; Home Theater existe e fica oculto na UI.
O `WearState` do vitrola **não** faz parte de preset (§3.6).

### 3.6 Adaptadores

**Vitrola (E19-07).** O módulo interno de vinil e válvula entra como `Node` por um
adaptador fino. Vinyl e Tube são dois módulos separados, cada um com `intensity` e
bypass próprio. O PRNG tem semente por faixa e por reprodução. O `WearState` é
serializável, versionado e persistido por álbum fora do preset; persistir é chamar o
repositório da biblioteca, não abrir tabela. Abaixo de 44,1 kHz o vitrola fica
`Unavailable` com `UnsupportedRate`. Em multicanal o vinil fica `Unavailable` com
`UnsupportedLayout` e a válvula roda por canal. No modo `Transparent` ou em caminho
DSD/DoP, o vitrola se desliga com `TransparentMode` ou `DsdPath`.

**Plugin (E12-07).** O VST3 roda num worker em processo próprio, falando com o grafo
por memória compartilhada limitada. O nó local declara a `latency_frames()` que o
plugin anunciou. Plugin travado ou morto põe o nó em bypass com `PluginHung`; a regra
de circuit breaker é de E07-04 e não é redefinida aqui.

## 4. Requisitos

| ID | Requisito | REQ de produto | TASK | Onde é provado |
|---|---|---|---|---|
| `NODE-REQ-01` | Todo nó implementa `Node`; `prepare` e `reset` rodam fora da RT, `process` não aloca nem trava — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-01 planejado) |
| `NODE-REQ-02` | Parâmetros chegam ao callback só por `ParamSnapshot` imutável trocado na fronteira do bloco, com interpolação dos `smoothable` — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-02 planejado) |
| `NODE-REQ-03` | `set_params` recusa valor fora de `ParamSpec` ou não finito sem alterar o snapshot vigente — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-03 planejado) |
| `NODE-REQ-04` | Bypass por nó e global é aplicado com crossfade, sem descontinuidade, e mantém a latência anunciada — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-04 planejado) |
| `NODE-REQ-05` | A cadeia segue `SLOT_ORDER`; `compile` recusa duplicata, autoconexão e ciclo — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-05 planejado) |
| `NODE-REQ-06` | Troca de cadeia é compilada fora da RT e entra por crossfade equal-power, sem alocação no callback — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-06 planejado) |
| `NODE-REQ-07` | `CompiledGraph::latency_frames()` é a soma das latências dos nós e alimenta o relógio apresentado — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-07 planejado) |
| `NODE-REQ-08` | Saída não finita ou instável põe o nó em bypass com `NonFinite`/`Unstable` e diagnóstico visível — planned (M2) | `REQ-E04-03` | `TASK-E04-03` | — (NODE-AC-08 planejado) |
| `NODE-REQ-09` | Preset inválido, corrompido ou de versão futura é recusado e o anterior continua ativo; preset incompatível com Fs ou canais é adaptado com aviso — planned (M1) | `REQ-E04-01` | `TASK-E04-01` | — (NODE-AC-09 planejado) |
| `NODE-REQ-10` | O vitrola entra como nó no slot `Vitrola`, com bypass, desativação automática com motivo e `WearState` fora do preset — planned (M3) | `REQ-E19-07` | `TASK-E19-07` | — (NODE-AC-10 planejado) |
| `NODE-REQ-11` | O vitrola é incorporado como módulo interno, sem dependência de repositório externo em build — planned (M3) | `REQ-E19-03` | `TASK-E19-03` | — (NODE-AC-11 planejado) |
| `NODE-REQ-12` | Plugin VST3 roda isolado; travar ou morrer põe o nó em bypass sem parar a reprodução — planned (M5) | `REQ-E12-07` | `TASK-E12-07` | — (NODE-AC-12 planejado) |

Os nomes de teste em §5 são rótulos descritivos; o nome final é fixado pela TASK e
registrado na coluna "Onde é provado".

## 5. Critérios de aceite

| ID | Given | When | Then | Teste |
|---|---|---|---|---|
| `NODE-AC-01` | grafo de 4 nós (`preamp` 0 dB, `peaking` 1 kHz Q 4,32 +3 dB, `compressor` attack 10 ms, `limiter` lookahead 96) a 48 kHz e blocos de 64 frames, numa thread de áudio falsa com contador de alocação | 10 000 blocos são processados com parâmetros mudando a cada 100 blocos | zero alocações e zero locks no callback | `graph_callback_never_allocates` |
| `NODE-AC-02` | o mesmo grafo com `peaking` em +3 dB | o ganho vai a −6 dB num único `set_params` | a resposta muda em rampa dentro da janela de smoothing, sem degrau maior que o da rampa | `param_swap_is_smoothed` |
| `NODE-AC-03` | `peaking` com faixa de ganho −30 a +40 dB | `set_params` recebe +41 dB, depois `NaN` | as duas chamadas devolvem `NodeError::OutOfRange`/`NonFinite` e o snapshot vigente não muda | `params_out_of_range_are_rejected` |
| `NODE-AC-04` | seno de 1 kHz passando pelo grafo com `limiter` (latência 96) | o `limiter` entra e sai de bypass 10 vezes | nenhuma descontinuidade acima do limiar do detector de E10-03 e `latency_frames()` constante | `bypass_is_click_free` |
| `NODE-AC-05` | um `GraphSpec` com 12 nós e 14 conexões, contendo uma conexão repetida, uma autoconexão e um ciclo de três nós | `compile` é chamado | devolve erro nomeando as três violações, sem cadeia parcial | `compile_rejects_invalid_graph` |
| `NODE-AC-06` | reprodução ativa com EQ `Graphic10` | o modo do EQ troca para `Peq32` | a troca usa crossfade equal-power, sem alocação no callback e sem descontinuidade | `chain_swap_is_crossfaded` |
| `NODE-AC-07` | cadeia com nós de latência 0, 96 e 64 frames | `latency_frames()` é lido e a posição apresentada é comparada com o consumo | `latency_frames()` = 160 e a posição apresentada descontada em 160 | `graph_latency_is_summed` |
| `NODE-AC-08` | nó de teste que emite `NaN` no bloco 50 | a reprodução continua | o nó passa a `Bypassed` com `NonFinite`, o áudio segue limpo e `node_status` expõe o motivo | `nonfinite_node_is_bypassed` |
| `NODE-AC-09` | preset válido aplicado; depois um preset com checksum errado e outro com `schema_version` futura | `apply_preset` é chamado com cada um | ambos recusados, o preset válido continua ativo e o erro é exposto | `bad_preset_keeps_previous` |
| `NODE-AC-10` | vitrola ativo a 48 kHz estéreo | o caminho passa a `Transparent`, depois a DSD, depois volta a processado a 32 kHz | o vitrola sai com `TransparentMode`, `DsdPath` e `UnsupportedRate`, com crossfade, e o `WearState` não aparece no preset salvo | `vitrola_auto_off_reasons` |
| `NODE-AC-11` | a árvore do engine num checkout limpo sem rede | `cargo build --locked` | compila sem buscar o repositório `vitrola` | `vitrola_is_vendored` |
| `NODE-AC-12` | um VST3 de teste que entra em laço em `process` | a reprodução de `sine-f32le.wav` continua por 5 s | o nó vai a `Bypassed` com `PluginHung`, sem underrun no stream | `hung_plugin_is_bypassed` (ignore) |

**Nenhum destes testes existe ainda.** Os valores numéricos de NODE-AC-01 e NODE-AC-05
vêm de `TASK-E04-03`; os de NODE-AC-10, de `TASK-E19-07`.

### Critérios que esta spec **não** fixa

- A duração exata do crossfade de bypass e de troca de cadeia dentro de 10 a 50 ms
  (fixada em E04-03 com medição).
- O limiar numérico de descontinuidade: é do harness de E10-03.
- A resposta em frequência de cada nó: é de `equalizadores.md` e `catalogo-dsp.md`.
- Os VIT-T-01..10 do vitrola: são de E19-07 e vivem em `tests/fixtures/vit/`.

## 6. Erros e cancelamento

**Erro é dado, não `panic!`.** Falhas do grafo usam um tipo próprio, convertido em
`crate::Error::Dsp` (`engine.md`) quando atravessa a fronteira pública:

```rust
pub enum NodeError {
    OutOfRange { node: NodeId, param: String },
    NonFinite { node: NodeId, param: String },
    PrepareFailed { node: NodeId, reason: String },
    InvalidGraph(Vec<GraphViolation>),
    UnsupportedRate { node: NodeId, rate: u32 },
    UnsupportedLayout { node: NodeId },
    PresetInvalid { reason: String },
    PresetFutureVersion { found: u32, supported: u32 },
}
```

| Situação | Efeito |
|---|---|
| `prepare` falha num nó | o nó entra `Bypassed` com `PrepareFailed`; a cadeia compila sem ele e o evento `Error::Dsp` é emitido |
| saída `NaN`/`Inf` ou filtro instável | o nó vai a `Bypassed` com crossfade; o bloco atual é substituído pelo da entrada |
| plugin travado ou morto | `Bypassed` com `PluginHung`; a reprodução não para |
| `compile` recebe grafo inválido | erro com todas as violações; a cadeia vigente continua |
| preset recusado | o preset anterior continua; o erro é exposto ao player |

Nenhuma dessas situações para a reprodução. Erro no callback nunca é propagado por
`Result`: vira estado do nó, lido fora da RT.

**Cancelamento é explícito, não timeout.** Uma compilação de cadeia em andamento é
abandonada quando a geração muda (seek, troca de faixa, stop); o resultado de uma
geração antiga é descartado sem ser publicado. O único prazo é o do worker de plugin,
cujo valor é de E07-04.

## 7. Fora de escopo

Vinyl/Tube e VST3 entram como modulos internos deste grafo (E19-07, E12-07). Jobs persistidos (E08-01) entram como infraestrutura nao-audio, decisao registrada

## 8. Dependências

Depende de: decode, encode, output, dsp (as quatro)

É dependido por: player, converter, restore

## 9. Testes e fixtures

- **Fixtures:** `tests/fixtures/graph/` recebe por cópia `sine-s24le.wav` e
  `sine-6ch-s24le.wav` de `libs/decode/tests/fixtures/pcm`, com SHA-256 conferido
  contra o `MANIFEST.txt` daquele diretório. O vitrola usa `tests/fixtures/vit/`
  (senos de 1, 5, 10 e 15 kHz a −20, −6 e 0 dBFS em 44,1 a 384 kHz).
- **Thread de áudio falsa:** consome blocos em memória com contador de alocação
  global; nenhum teste abre dispositivo.
- **Oráculos:** soma de latências conferida contra deslocamento medido por
  correlação cruzada; descontinuidade medida pelo harness de E10-03.
- **Plugin:** `hung_plugin_is_bypassed` e os demais testes VST3 ficam atrás de
  `#[ignore = "requer VST3 de teste compilado do SDK pinado; VYNAMP_VST3_FIXTURES"]`,
  com a variável `VYNAMP_VST3_FIXTURES` apontando para os `.vst3` de teste.
- **Comando:** `cd libs/engine && cargo test --test graph --locked`.

## 10. Limitações conhecidas

Limitação declarada não reprova a entrega; limitação omitida reprova.

| Limitação | Efeito | Quem cobre |
|---|---|---|
| Grafo é cadeia de ordem fixa, não grafo livre | o usuário não reordena slots | D-DSP-10 e D-DSP-12 |
| Com o vitrola ligado, o crossfade entre faixas pode ser desativado (D-DSP-06, candidato) | transição cai para gapless ou corte | decisão D-DSP-06 |
| Vitrola indisponível abaixo de 44,1 kHz e vinil indisponível em multicanal | o nó aparece `Unavailable` com motivo | E19-07 |
| VST3 só em M5 e só desktop | mobile não carrega plugin | E12-07 |
| Grafo interno é `f32`; nós `f64` convertem localmente | custo de conversão por bloco | E12-10 |
