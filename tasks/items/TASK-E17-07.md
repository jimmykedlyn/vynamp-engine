# TASK-E17-07 — Fixar proteção true-peak depois do processamento

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E17-07` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E17-07` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E04-03`, `TASK-E04-07` |
| Bloqueia | `TASK-E17-08` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 2–5 dias úteis |
| Risco | alto |

## Entrega

Guard final após mix/master/resampler e antes da quantização, separado do limiter criativo.

## Boundary

- **Não** é o limiter criativo nem o compressor. Threshold, ratio, knee, attack, release, makeup, reduction meter, bass, treble, mono e routing com meters são `TASK-E04-07`, e o sample-peak limiter e o soft clipping têm de receber outro nome; se este nó ganhar knob de release ou de redução visível, o usuário passa a achar que desligar o limiter criativo desliga a proteção.
- **Não** mede o teto no PCM final com oráculo independente. O harness que captura o buffer do backend e roda o estimador de referência sobre o .raw, com 5.1 e as tolerâncias por bit-depth, é `TASK-E17-08`; esta TASK entrega o nó e a política, e o teste aqui é o do nó, não o do PCM entregue ao backend.
- **Não** implementa o modo transparente nem o caminho inteiro. O integer direct path, as condições ativas e o selo bit-perfect são `TASK-E04-06`, em vynamp-output; aqui só se garante que o guard aceita ser bypassado nesse estado e que não aparece protegido com o nó final bypassado.
- **Não** projeta o resampler nem a sample conversion. Taxa de saída, latência de rate change e dither só ao reduzir precisão são `TASK-E04-02`, em vynamp-dsp; o guard fica depois do resampler e antes da quantização, e um overs de resample vira evidência para aquela TASK, nunca margem a mais no ceiling.
- **Não** constrói o grafo, a anotação de latência nem o crossfade. Ordem de nós compilada, compensação de latência e swap por generation fora da RT são `TASK-E04-03`; esta TASK consome a anotação que ele publica e declara o lookahead como número, sem reimplementar o agendamento.
- **Não** é a state machine nem o contador de generation. Incrementar a geração e descartar buffer de geração velha são `TASK-E01-03`; drenar o lookahead no seek é do guard, mas quem invalida a geração é o domínio, e Stop idempotente não se prova aqui.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e17-07-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E17-07-AC-1` | o grafo compilado de TASK-E04-03 com EQ de +15 dB em três bandas, preamp de +6 dB, ganho master 1,0, crossfade equal-power entre duas fontes correlacionadas e resampler de 44,1 para 48 kHz, e o guard true-peak em -1 dBTP como último nó DSP antes da quantização | a ordem de nós é despejada em texto e o harness tenta inserir um boost de +3 dB e um ganho de master de 1,5 depois do guard, antes de o buffer seguir para a quantização | Nenhum boost/processamento posterior |
| `TASK-E17-07-AC-2` | um perfil novo, sem ceiling escolhido pelo usuário, sem headroom manual e com o guard habilitado, e um trem de impulsos estéreo correlacionado em 0 dBFS mais um sweep de 20 Hz a 20 kHz a 48 kHz | o ceiling efetivo é lido da configuração do nó em dBTP e o pico inter-sample do buffer de saída do guard é medido por oversampling 4x com filtro FIR de referência | ceiling padrão -1 dBTP |
| `TASK-E17-07-AC-3` | o guard ativo com lookahead pinado no fixture M0, o buffer de lookahead do guard cheio com amostras da generation 1 e o modo transparente disponível na mesma cadeia | a latência do nó é lida em amostras e em milissegundos na taxa de saída, um seek incrementa a geração para 2 e a cadeia alterna do modo processado para o transparente e de volta | latency/reset/generation e modo transparente coerentes. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | teste de integração novo em libs/engine/tests/guard_true_peak.rs, com fixture próprio de trem de impulsos estéreo correlacionado em 0 dBFS e sweep de 20 Hz a 20 kHz a 48 kHz, montando EQ de +15 dB em três bandas, preamp de +6 dB, master 1,0, crossfade equal-power, resampler de 44,1 para 48 kHz e o guard em -1 dBTP, com dump da ordem de nós em texto. Hoje o repo vynamp-engine tem só src/lib.rs com 6 linhas e o único símbolo é VERSION, sem diretório tests e sem fixtures, então o teste nem compila. | o dump da ordem mostra o guard como último nó DSP, com apenas quantização e dither depois dele, e inserir boost ou ganho 1,5 depois é recusado com erro de grafo; o pico inter-sample do buffer de saída do guard, medido por oversampling 4x com FIR de referência, fica em -1,00 dBTP ou abaixo, isto é 0,8913 de escala cheia, com teto absoluto de -0,90 dBTP (0,9016) para o alvo mais 0,1 dB; o ceiling lido de um perfil novo é -1,0 dBTP; a latência publicada é o lookahead do fixture M0 em frames multiplicado por 1000 e dividido por Fs, e o lookahead retido da generation 1 é zero amostras depois do seek. O que não existe hoje: busca por true_peak, dbtp e oversampl nos 168 arquivos .rs do umbrella devolve 0 linhas, vynamp_engine não declara dependência alguma em Cargo.toml e nenhum valor de lookahead está pinado em fixture, então todo número acima ainda é meta, não medição. |
| Hardware | nenhum: o guard opera em buffer float e a verificação de teto roda sobre o buffer em memória ou sobre o .raw gravado pelo Capture host de libs/output/crates/vynamp-output-core/src/capture.rs com o .manifest ao lado, sem abrir dispositivo de áudio; a captura de DAC que a QA-027 cita como complemento não existe nesta máquina Windows, sem placa pro, sem Android, sem iOS e sem macOS | ausência é limitação registrada, não aprovação |

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
