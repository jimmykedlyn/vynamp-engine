# Spec — vynamp-engine · engine

| Campo | Valor |
|---|---|
| Módulo | `vynamp-engine` |
| Crate(s) | `vynamp-engine` (crate único até uma TASK justificar divisão) |
| Tipo | lib |
| Versão da spec | 0.2.0 |
| Data | 2026-11-11 |
| Estado | **rascunho autorado, não aprovado** — §3 a §6, §9 e §10 escritos em 2026-11-11 a partir de `reference/audio/` e das TASKs E01-03, E01-05, E01-06, E01-10 e E08-01. O crate hoje só publica `VERSION`; tudo aqui é desenho de API. A aprovação é do mantenedor, por `GOVERNANCE.md` |
| Dono | mantenedor do `vynamp-engine` |
| Depende de | decode, encode, output, dsp (as quatro) |
| É dependido por | player, converter, restore |

> §1, §2 e §7 foram decididos em `01-arvore-final.md` §2 e valem como norma. §3 a §6,
> §9 e §10 foram autorados em 2026-11-11 a partir da referência de áudio do umbrella
> (pipeline, threads, estados e metas) e dos critérios das TASKs do engine. Estar
> escrito não é estar aprovado. Como o crate ainda é um stub, **todo requisito está
> `planned`** e nenhum teste existe: os critérios de §5 nomeiam o teste que a TASK
> correspondente cria. Esta spec é o contrato de domínio (tipos, estados, comandos,
> eventos, cancelamento); o caminho de amostras está em [`pipeline.md`](pipeline.md) e
> o grafo DSP em [`dsp.md`](dsp.md).

## 1. Propósito

Integrar decode, dsp, output e encode num grafo de reproducao com estado, comandos e grafo DSP.

**Não faz:** Nao tem UI, nao gerencia biblioteca, nao abre dispositivo por conta propria sem o output.

## 2. Fronteira

| Permitido | Proibido |
|---|---|
| decode, encode, output, dsp (as quatro, obrigatorias) | qualquer app; qualquer crate de dominio externo a estas quatro libs |

## 3. API pública

> Autorado em 2026-11-11 e **não aprovado**. É o desenho que a TASK-E01-03 implementa;
> um nome pode mudar na implementação desde que a mudança volte a esta seção no mesmo PR.

### 3.1 Tipos de tempo e formato

```rust
pub struct SampleRate(pub u32);                      // Hz, > 0
pub struct FrameCount(pub u64);                      // frames por canal
pub struct MediaTime { ticks: i64, timescale: u32 }  // tempo racional, sem f64
pub use vynamp_decode::ChannelLayout;
```

- `MediaTime` é a única representação de posição no domínio. A conversão
  `MediaTime::to_frames(SampleRate) -> FrameCount` arredonda para baixo e não passa
  por ponto flutuante.
- `ChannelLayout` é o do `vynamp-decode`, reexportado; o engine não define o seu.

### 3.2 Estados

```rust
pub enum PlaybackState {
    Idle, Opening, Buffering, Playing, Paused, Seeking, Stopping,
    Reconnecting, DeviceLost, Error(ErrorKind),
}
```

Transições permitidas (qualquer outra é recusada com `Error::InvalidTransition`):

| De | Para | Gatilho |
|---|---|---|
| Idle | Opening | `Command::Open` |
| Opening | Buffering | fonte demuxada e decoder pronto |
| Buffering | Playing / Paused | ring acima da marca de partida; vai a Paused se o pedido foi `Open { autoplay: false }` |
| Playing | Paused | `Command::Pause` |
| Paused | Playing | `Command::Play` |
| Playing / Paused | Seeking | `Command::Seek` |
| Seeking | Buffering | ring da geração nova começou a encher |
| qualquer estado ativo | Stopping | `Command::Stop` ou erro fatal |
| Stopping | Idle | geração encerrada (ver §6: worker preso é abandonado, não esperado) |
| Playing / Paused | Reconnecting | `vynamp-output` reporta perda de rota recuperável |
| Reconnecting | Playing / Paused | rota reaberta; volta ao estado anterior |
| Reconnecting | DeviceLost | prazo de reconexão esgotado |
| qualquer | Error | erro não recuperável; sai só por `Stop` → Idle |

### 3.3 Comandos e eventos

```rust
pub enum Command {
    Open { source: SourceRef, start: MediaTime, autoplay: bool },
    Play, Pause, Stop,
    Seek(MediaTime),
    Enqueue(SourceRef),          // próxima faixa, para gapless ou crossfade
    SetVolume(f32),              // master 0.0..=1.0; nunca amplifica
    SetTransition(Transition),   // None | Gapless | Crossfade { duration }
}

pub enum Event {
    StateChanged { from: PlaybackState, to: PlaybackState, generation: Generation },
    TrackStarted { generation: Generation, source: SourceRef },
    TrackEnded { generation: Generation },
    GaplessDegraded { generation: Generation },  // junção sem metadado de trim
    Underrun { count: u64 },
    Error(Error),
}
```

- `Engine::send(Command) -> Result<(), Error>` valida, enfileira e devolve; nunca
  bloqueia esperando o áudio.
- Eventos saem por canal limitado. Evento perdido por canal cheio incrementa
  `Snapshot::dropped_events`; a callback de áudio nunca bloqueia por isso.
- `SetVolume` acima de 1.0, negativo ou NaN é recusado; o padrão é 1.0 (sem ganho).
- `SourceRef` é o handle de fonte do `vynamp-decode` (caminho ou stream já aberto).

### 3.4 Geração e cancelamento

```rust
pub struct Generation(u64);
pub struct CancelToken { /* compartilhado pelos workers de uma geração */ }
impl CancelToken { pub fn is_cancelled(&self) -> bool; }
```

- `Open`, `Seek`, troca de faixa e `Stop` incrementam `Generation` e cancelam o
  `CancelToken` da geração anterior.
- Buffer produzido por geração velha é descartado pelo consumidor sem ser apresentado.
- `Stop` em `Idle` não muda estado nem emite evento.

### 3.5 Snapshot

```rust
pub struct Snapshot {
    pub state: PlaybackState,
    pub generation: Generation,
    pub position: MediaTime,      // posição apresentada, ver pipeline.md §3.3
    pub underruns: u64,
    pub dropped_events: u64,
}
impl Engine { pub fn snapshot(&self) -> Snapshot; }
```

A UI lê só snapshots; nunca recebe referência para estado interno.

### 3.6 Jobs persistidos

O scheduler da TASK-E08-01 vive neste crate como infraestrutura não-áudio (§7): fila
com checkpoint em disco, lease por job, progresso e pools limitados. Os nomes
(`JobQueue`, `JobId`, `JobState`, `Checkpoint`) e as assinaturas são fixados pela
TASK e voltam a esta seção quando ela sair de `planned`; aqui fica só a garantia de
ENGINE-REQ-12.

## 4. Requisitos

> Autorado em 2026-11-11. **Não aprovado.** Cada requisito aponta para o critério de §5
> que o prova; `planned` marca o que ainda não tem código, e aqui é tudo, porque o
> crate é um stub.

| ID | Requisito | REQ de produto | TASK | Onde é provado |
|---|---|---|---|---|
| ENGINE-REQ-01 | Máquina de estados de §3.2; transição fora da tabela é recusada sem mudar o estado. — planned (M1) | REQ de controle de reprodução | E01-03 | — (ENGINE-AC-01 planejado) |
| ENGINE-REQ-02 | Stop durante Opening cancela todo o trabalho em voo. — planned (M1) | REQ de controle de reprodução | E01-03 | — (ENGINE-AC-02 planejado) |
| ENGINE-REQ-03 | Nova geração cancela todos os workers da anterior antes do primeiro buffer novo. — planned (M1) | REQ de controle de reprodução | E01-03 | — (ENGINE-AC-03 planejado) |
| ENGINE-REQ-04 | Buffer de geração velha nunca chega à saída. — planned (M1) | REQ de reprodução fiel | E01-03 | — (ENGINE-AC-04 planejado) |
| ENGINE-REQ-05 | `Stop` é idempotente. — planned (M1) | REQ de controle de reprodução | E01-03 | — (ENGINE-AC-05 planejado) |
| ENGINE-REQ-06 | Fim de faixa drena até o último frame de conteúdo; pause/stop não deixam áudio velho no próximo play. — planned (M1) | REQ de reprodução fiel | E01-05 | — (ENGINE-AC-06, ENGINE-AC-07 planejados) |
| ENGINE-REQ-07 | Volume padrão 1.0 e nunca acima disso; A/B entre fontes iguais não deriva. — planned (M1) | REQ de reprodução fiel | E01-05 | — (ENGINE-AC-08 planejado) |
| ENGINE-REQ-08 | Gapless com trim de encoder delay/padding; sem metadado, emite `GaplessDegraded`. — planned (M2) | REQ de gapless e crossfade | E01-06 | — (ENGINE-AC-09 planejado) |
| ENGINE-REQ-09 | Latência de comando: open p95 ≤ 350 ms, seek p95 ≤ 150 ms, play/pause p95 ≤ 80 ms. — planned (M3) | REQ de resposta da interface | E01-10 | — (ENGINE-AC-10 planejado) |
| ENGINE-REQ-10 | 8 h de reprodução contínua sem underrun e sem crescimento de memória. — planned (M3) | REQ de reprodução sem falhas | E01-10 | — (ENGINE-AC-11 planejado) |
| ENGINE-REQ-11 | Decoder travado não segura Stop nem troca de faixa. — planned (M3) | REQ de robustez | E01-10 | — (ENGINE-AC-12 planejado) |
| ENGINE-REQ-12 | Jobs retomam do último checkpoint após crash; cancelar apaga só temporários, nunca a fonte; falha de um job não para a fila. — planned (M2) | REQ de conversão e restauração em lote | E08-01 | — (ENGINE-AC-13 planejado) |
| ENGINE-REQ-13 | Perda de dispositivo leva a `Reconnecting` e, esgotado o prazo, a `DeviceLost`, sem `panic!`. — planned (M2) | REQ de robustez | E01-05 | — (ENGINE-AC-14 planejado) |
| ENGINE-REQ-14 | Nenhuma entrada provoca `panic!`; depois de qualquer erro de §6 a instância aceita `Stop` e volta a `Idle`. — planned (M1) | REQ de robustez | E01-03 | — (ENGINE-AC-15 planejado) |

Os rótulos de REQ de produto são descritivos; trocar cada um pelo ID formal do
documento de requisitos do umbrella faz parte da aprovação.

## 5. Critérios de aceite

> Autorado em 2026-11-11. **Não aprovado.** Cada linha resume o critério da TASK
> citada; havendo divergência, vale o texto da TASK, e esta tabela é corrigida no mesmo
> PR. **Nenhum destes testes existe ainda**; a coluna nomeia o teste que a TASK cria.

| ID | Given | When | Then | Teste |
|---|---|---|---|---|
| `ENGINE-AC-01` | o engine em cada estado de §3.2 | cada comando de §3.3 é enviado | transição fora da tabela devolve `Error::InvalidTransition` e o estado não muda | `state_table_is_closed` |
| `ENGINE-AC-02` | `Opening` com worker de I/O e produtor de decode vivos | `Stop` chega antes do primeiro buffer | todos os workers terminam ou são abandonados e o estado chega a `Idle` (TASK-E01-03-AC-1) | `stop_during_opening_cancels_everything` |
| `ENGINE-AC-03` | uma geração com o mesmo `CancelToken` em N workers | a geração é incrementada | `is_cancelled()` é verdadeiro nos N antes do primeiro buffer da geração nova | `new_generation_cancels_old_workers` |
| `ENGINE-AC-04` | `Playing` com buffers da geração 1 no ring | seek leva a geração de 1 a 2 | nenhum frame da geração 1 é apresentado depois do seek (TASK-E01-03-AC-2) | `seek_discards_stale_buffers` |
| `ENGINE-AC-05` | o engine em `Idle` após `Stop` | `Stop` é enviado mais três vezes | estado e contador de eventos não mudam (TASK-E01-03-AC-3) | `repeated_stop_is_idempotent` |
| `ENGINE-AC-06` | faixa com número de frames que não é múltiplo do período | a reprodução chega ao EOF | o último frame de conteúdo é apresentado (TASK-E01-05) | `eof_drains_to_last_frame` |
| `ENGINE-AC-07` | pausado com amostras no ring e decode em voo | Stop, Open de outra faixa e Play | nenhuma amostra da faixa anterior sai (TASK-E01-05) | `stop_then_play_has_no_stale_audio` |
| `ENGINE-AC-08` | duas cópias do mesmo arquivo, volume padrão | A/B alternado por 60 s | posição e frames apresentados não derivam; nenhuma amostra excede a da fonte (TASK-E01-05) | `ab_switch_has_no_drift_and_no_gain` |
| `ENGINE-AC-09` | os pares FLAC/MP3/AAC da TASK-E01-06, incluindo `nolame-cbr128-48k.mp3` (delay 529) e FLAC com `total_samples` zerado | tocados em sequência | nos pares com metadado a junção é igual ao original; nos sem metadado sai `GaplessDegraded` | `gapless_pairs_rejoin` |
| `ENGINE-AC-10` | o harness de latência da TASK-E01-10 | 1 000 repetições de open, seek e play/pause | os p95 ficam dentro de ENGINE-REQ-09 | `command_latency_p95` (`#[ignore]`) |
| `ENGINE-AC-11` | FLAC 24/192 sine 997 Hz de 8 h (TASK-E01-10) | reprodução contínua | zero underruns e memória residente estável | `soak_8h` (`#[ignore]`) |
| `ENGINE-AC-12` | decoder de teste bloqueado em `Condvar` sem prazo | Stop e, noutra passada, troca de faixa | o engine responde dentro do prazo medido, apesar do worker preso (TASK-E01-10) | `stuck_decoder_does_not_hold_stop` |
| `ENGINE-AC-13` | fila de 40 jobs com crash no 12; um cancelamento; 30 jobs com 4 falhas distintas (TASK-E08-01) | a fila roda | retoma do checkpoint; cancelar remove o temporário e preserva a fonte; os 26 bons terminam e as 4 falhas são reportadas | `jobs_resume_cancel_and_isolate_failures` |
| `ENGINE-AC-14` | `NullHost` do `vynamp-output` configurado para perder a rota | a perda é injetada e a rota não volta | `Reconnecting` → `DeviceLost`, sem `panic!` | `device_loss_goes_to_device_lost` |
| `ENGINE-AC-15` | cada variante de erro de §6 provocada | a chamada falha | o engine aceita `Stop` e volta a `Idle` | `errors_are_data_and_engine_stays_usable` |

### Critérios que esta spec **não** fixa

- **Apresentação de eventos.** O player decide como mostrar; o engine garante só a
  ordem e a geração de cada evento.
- **Fila, shuffle e repeat.** São do player; o engine conhece só a faixa atual e a
  próxima (`Enqueue`).
- **Prazo exato de reconexão e de abandono de worker.** Fica como configuração; o valor
  padrão é fixado pela TASK-E01-10 depois da medição.

## 6. Erros e cancelamento

**Erro é dado, não `panic!`.** Toda operação pública devolve `Result<_, Error>`.

```rust
#[non_exhaustive]
pub enum Error {
    InvalidTransition { from: PlaybackState, command: CommandKind },
    InvalidArgument(&'static str),   // volume fora de 0..=1, seek além da duração
    Decode(vynamp_decode::Error),
    Output(vynamp_output::Error),
    Dsp(vynamp_dsp::Error),
    DeviceLost,
    Cancelled,
    Io(std::io::ErrorKind),
}
```

| Variante | Quando | O que o chamador faz |
|---|---|---|
| `InvalidTransition` | comando que §3.2 não aceita no estado atual | ignora ou ajusta a UI; o estado não mudou |
| `InvalidArgument` | parâmetro fora do domínio | corrige o pedido |
| `Decode` | fonte corrompida ou formato não suportado | mostra o erro e oferece a próxima faixa |
| `Output` | falha ao abrir ou manter o dispositivo | tenta outro dispositivo ou modo |
| `Dsp` | nó recusou `prepare` | nada; o nó segue em bypass (ver [`dsp.md`](dsp.md) §6) e o erro é só informativo |
| `DeviceLost` | prazo de reconexão esgotado | pede ao usuário outro dispositivo |
| `Cancelled` | operação abandonada por nova geração ou `Stop` | nada; não é falha |
| `Io` | leitura da fonte ou do checkpoint de job falhou | mostra o erro |

**Cancelamento é explícito, não timeout.** Ele só acontece por `Stop`, `Seek`, `Open`
ou troca de faixa, que incrementam a geração e cancelam o `CancelToken`. O engine não
espera o worker cancelado para seguir: a geração nova começa na hora e o worker velho
é abandonado e recolhido quando retornar (ENGINE-REQ-11). O prazo de §3.2 em
`Reconnecting` é a única espera com relógio, e ele não cancela nada: só decide entre
voltar a tocar e `DeviceLost`. A callback de áudio nunca observa cancelamento; ela só
compara a geração do buffer.

## 7. Fora de escopo

Vinyl/Tube e VST3 entram como modulos internos deste grafo (E19-07, E12-07). Jobs persistidos (E08-01) entram como infraestrutura nao-audio, decisao registrada

## 8. Dependências

Depende de: decode, encode, output, dsp (as quatro)

É dependido por: player, converter, restore

## 9. Testes e fixtures

- **Fixtures de áudio:** reusa as de `libs/decode/tests/fixtures/` com hash conferido
  contra o `MANIFEST.txt` de cada diretório; o engine não duplica arquivo. Os pares de
  gapless da TASK-E01-06 entram lá, não aqui.
- **Sem hardware:** todo teste de §5 roda sobre o `NullHost` de `vynamp-output` com
  relógio simulado, exceto `command_latency_p95` e `soak_8h`.
- **Arquivos grandes:** o FLAC de 8 h é gerado pelo comando da TASK-E01-10, fica fora
  do git, e o teste vive atrás de `#[ignore = "soak de 8 h; precisa de VYNAMP_SOAK_FIXTURE"]`
  com a variável `VYNAMP_SOAK_FIXTURE` apontando para o arquivo.
  `command_latency_p95` fica atrás de `#[ignore]` porque mede hardware real.
- **Oráculos:** comparação bit a bit para gapless e drenagem; contagem de frames por
  geração para descarte; percentis medidos para latência; RSS amostrado a cada minuto
  para o soak.

## 10. Limitações conhecidas

Limitação declarada não reprova a entrega; limitação omitida reprova.

| Limitação | Efeito | Quem cobre |
|---|---|---|
| Crate ainda é stub | nenhum requisito implementado | TASK-E01-03 em diante |
| Worker preso é abandonado, não morto | thread de decode travada segura memória até retornar | TASK-E01-10 mede; isolamento por processo só para VST3 (TASK-E12-07) |
| Gapless sem metadado de trim | a junção pode ter o silêncio do encoder | declarado por `GaplessDegraded`; ENGINE-REQ-08 |
| Metas de latência medidas numa máquina de referência | o número não vale para qualquer hardware | relatório de suporte por perfil da TASK-E01-10 |
| API de jobs não fixada aqui | contrato só na TASK-E08-01 | volta a §3.6 ao implementar |
