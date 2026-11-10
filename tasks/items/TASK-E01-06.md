# TASK-E01-06 — Implementar gapless sample-accurate

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E01-06` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E01-06` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M2 |
| Prioridade | P1 |
| Estado | planned |
| Depende de | `TASK-E01-05` |
| Bloqueia | `TASK-E01-10` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 5–12 dias úteis |
| Risco | alto |

## Entrega

Predecode next e trim validado de encoder delay/padding.

## Boundary

- **Não** implementa o mixer, o dreno de EOF nem o ramp de play/stop. `TASK-E01-05` entrega isso e esta TASK depende dela: aqui o último frame da faixa A tem que chegar inteiro ao backend e a faixa B começar no frame seguinte, sem refazer o dreno.
- **Não** deriva priming/preskip do container. `Track::delay`, `Track::padding` e `GaplessKind` já vêm preenchidos pelo `vynamp-decode`, e os contratos de trim, gain e timeline por codec (Opus/MP4/Matroska/Ogg) são `TASK-E17-02`; o engine só decide cortar ou não cortar com o que recebeu.
- **Não** implementa os adapters AAC/ALAC/Vorbis/Opus. `TASK-E01-07` é dona deles: hoje `vynamp-decode` tem 9 crates e nenhum de AAC, então o par AAC do critério só passa a existir quando esse adapter existir, e inventar trim de AAC aqui seria usar metadado que ninguém parseia.
- **Não** faz crossfade nem transição equal-power. `TASK-E04-03` é dona do grafo compilado e do crossfade de 0–12 s; a spec de `01-engine.md` §Gapless diz que gapless e crossfade são políticas mutuamente exclusivas na mesma fronteira, então escolher a política é desta TASK e executar a outra não é.
- **Não** reabre o dispositivo nem negocia taxa para preservar o gapless. A troca de sample rate em modo exclusivo pode quebrar a emenda sem corte, e reabrir/negociar é `TASK-E04-05`; esta TASK expõe a quebra como limitação declarada em vez de reamostrar para escondê-la.
- **Não** publica o relatório de suporte por perfil nem roda o soak de 8 h. Medir 8 h a 24/192, cancelar decoder travado e publicar as limitações é `TASK-E01-10`; aqui o gapless é provado em pares curtos e a limitação de troca de taxa só fica registrada para aquele relatório.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e01-06-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E01-06-AC-1` | três pares consecutivos cortados do mesmo WAV de referência em duas metades cada, um par por formato: FLAC estéreo 16 bits/44,1 kHz, MP3 Layer III 128 kbps 48 kHz e AAC-LC de 1024 amostras por raw data block com iTunSMPB. O par FLAC segue o formato de `st16-44k1.flac` de `libs/decode/tests/fixtures/flac` (SHA-256 e `total_samples` no `MANIFEST.txt` daquele diretório), mas aquele arquivo tem só 4 410 frames, 0,1 s, então não serve de par: o predecode de 2 s esgotaria a faixa inteira. O par MP3 usa o formato de `cbr128-48k-stereo.mp3` de `libs/decode/crates/vynamp-decode-mpa/tests/fixtures`, onde o arquivo inteiro tem 126 pacotes de 1152 amostras, 145 152 frames, `Track::delay` 1 105 e `Track::padding` 47, com LAME tag de CRC válido | o engine reproduz cada par em sequência com gapless ligado e predecode da próxima faixa de pelo menos 2 s, e os frames entregues ao backend de captura são comparados com a concatenação do PCM decodificado das duas faixas, sem corte | Pair FLAC/MP3/AAC válido concatena sem frames extras/perdidos |
| `TASK-E01-06-AC-2` | um par de faixas sem metadado de trim: a primeira é `nolame-cbr128-48k.mp3` (126 pacotes de 1152 amostras, `Track::delay` 529, `padding` 0, `frames` `None` e `gapless` `Unknown`, como o teste de `libs/decode/crates/vynamp-decode-mpa/tests/demux.rs` fixa) e a segunda é uma FLAC com os 36 bits de `total_samples` do STREAMINFO zerados — nenhum `.flac` do repo tem isso, todos os de `libs/decode/tests/fixtures/flac` e de `libs/decode/crates/vynamp-decode-flac/tests/fixtures` declaram o total, então o arquivo é sintetizado a partir de um deles | o engine emenda o par com gapless ligado e a saída capturada é medida no ponto de emenda e no total de frames entregues | missing trim metadata não usa silêncio heurístico. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --test gapless` sobre `tests/fixtures/gapless/`, que recebe por cópia o `.flac` de `libs/decode/tests/fixtures/flac` (o único diretório de fixture do decode com `MANIFEST.txt` para FLAC, SHA-256 conferido contra ele) e `cbr128-48k-stereo.mp3` e `nolame-cbr128-48k.mp3` de `libs/decode/crates/vynamp-decode-mpa/tests/fixtures`, que não tem `MANIFEST.txt` nenhum, só um `.gitattributes` com `* -text`, então o hash dos dois MP3 tem que nascer no MANIFEST novo de `tests/fixtures/gapless/`; o `CaptureHost` de `libs/output` grava os frames em `tests/out/`, a referência é o PCM das duas faixas decodificado e concatenado sem corte, e `ffprobe -count_packets` fixa o número de pacotes de cada arquivo | Esperado: diferença de 0 frames entre o capturado e a referência nos dois pares medidos; no par MP3, os 145 152 frames de 1152 amostras menos `Track::delay` 1 105 (`encoder_delay` 576 + 529) e `Track::padding` 47 (`padding` 576 do LAME tag menos 529) dão exatamente os 144 000 frames de PCM encodado da tabela de `libs/decode/crates/vynamp-decode-mpa/tests/demux.rs`, e a emenda cai no frame 144 000 sem amostra repetida; no par sem metadado, `nolame-cbr128-48k.mp3` entrega os seus 145 152 frames sem tirar nada e nenhum silêncio é procurado, mas os dois `Track::frames` são `None` — `demux.rs` fixa `(529, 0, GaplessKind::Unknown)` para o arquivo sem LAME tag e `libs/decode/crates/vynamp-decode-flac/src/demux.rs` grava `(info.total_samples != 0).then_some(info.total_samples)` —, então o total esperado do par só sai do PCM real medido uma vez com `ffprobe`, nunca do STREAMINFO. Hoje `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, só `pub const VERSION`), `Cargo.toml` com `[dependencies]` vazio e nenhum `tests/`, então `cargo test --test gapless` falha por alvo inexistente; `CaptureHost` só existe atrás da feature `capture` de `vynamp-output-core`, exposta por `vynamp-output` como `test-capture` e desligada por default, e o engine não declara hoje dependência nenhuma de `libs/output`; §5, §6, §9 e §10 de `libs/engine/specs/engine.md` continuam sem conteúdo; e o par AAC não pode ser medido antes de `TASK-E01-07`. |
| Hardware | nenhum: a emenda é medida nos frames que o `CaptureHost` de `libs/output` grava em disco, sem abrir dispositivo; a quebra de gapless por troca de taxa em modo exclusivo exige DAC real e fica como limitação que `TASK-E04-05` executa e `TASK-E01-10` publica, e nem com DAC ela seria exercitada aqui, porque `libs/output` não tem backend de SO: `lib.rs` declara que só `NullHost` existe fora da feature de captura; este ambiente Windows não tem placa pro, aparelho Android, iOS nem macOS | ausência é limitação registrada, não aprovação |

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
