# TASK-E19-07 — Integrar o módulo DSP vitrola no grafo do player

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E19-07` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E19-07` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E19-03`, `TASK-E04-03` |
| Bloqueia | nenhuma |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 8–20 dias úteis |
| Risco | alto |

## Entrega

Módulo DSP interno de vinil e válvula integrado ao grafo do player, com bypass, desativação automática, persistência de WearState e os testes VIT-T-01..10 executáveis.

## Boundary

- **Não** compila o grafo, não troca cadeia fora da RT nem define a curva do crossfade. Isso é `TASK-E04-03`, que entrega o nó compilado, a anotação de latência e o crossfade equal-power; aqui se escreve o adaptador do vitrola nesse contrato e se prova a transparência depois do crossfade, não a máquina de transição. A confusão é plausível porque o AC-1 fala em crossfade e o bypass do vitrola é o primeiro uso dele.
- **Não** decide o modo transparente, não calcula `PathProperties` nem emite o selo bit-perfect. Essas condições e os diagnósticos hi-res são de `TASK-E04-06`; esta TASK apenas lê o estado do caminho, desliga o vitrola e publica o motivo no snapshot do nó.
- **Não** implementa DoP nem DSD nativo. A negociação e o framing DSD são de `TASK-E12-04`; aqui se consome o estado e se desativa com motivo. Trazer o framing para cá faria dois donos do caminho DSD e um selo que ninguém mediu.
- **Não** cria tabela, migration nem write actor da biblioteca. O schema SQLite é de `TASK-E03-01`; esta TASK entrega o `WearState` serializável, versionado e com migração própria (vitrola §8.8, decisão 6) e a chave por álbum, e persistir é chamar o que o schema expõe.
- **Não** define os mapas de canal 5.1 e 7.1 nem decodifica DSD para PCM. `TASK-E01-09` entrega o channel mapping; o layout multicanal chega pronto e o que se prova no VIT-T-03 é o comportamento do vitrola com ele — vinil indisponível com aviso e válvula por canal.
- **Não** implementa o limitador true-peak nem mede o teto do PCM final. O nó de proteção com teto padrão de −1 dBTP, lookahead anunciado e posição fixa é `TASK-E17-07`, e a medição contra ceiling +0,1 dB é `TASK-E17-08`; aqui se mede pico pós-vitrola e clipping por estágio, e o VIT-T-07 verifica que o teto continua respeitado.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e19-07-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E19-07-AC-1` | um bloco estéreo f32 a 48 kHz montado de `libs/decode/tests/fixtures/pcm/sine-f32le.wav` (4 800 frames, 2 canais, SHA-256 `31f0bf588db4068b70a2f7702824bfbc908d3acbeba273455989a052fd829aaa` no `MANIFEST.txt` daquele diretório) repetido até 30 s e seguido de 2 s de silêncio digital, com o vitrola ligado (vinil e válvula) em intensidade 60% e semente fixa `0x5EED`, inserido como nó no grafo compilado de `TASK-E04-03` entre o EQ e o ganho master | o bypass do vitrola é acionado durante a reprodução com crossfade de 20 ms e, terminado o crossfade, o bloco entregue ao backend é comparado amostra a amostra com a mesma entrada processada sem o nó na cadeia, com um detector de degrau varrendo a fronteira da transição | Bypass transparente após o crossfade |
| `TASK-E19-07-AC-2` | o vitrola ativo (vinil e válvula, intensidade 60%) a 48 kHz estéreo em modo processado, com o estado do nó e o motivo publicados no snapshot de status que o player lê, e um pedido de política de saída que alterna para bit-perfect e, numa segunda passada, para DSD64 em DoP, exercitado sobre o `NullHost` de `libs/output`, que aceita a política sem dispositivo real | o engine troca a política de saída no meio da reprodução, com áudio em andamento e sem parar o transporte, e a cada troca o snapshot de status do vitrola é lido e comparado com o estado esperado do nó | desativação automática em bit-perfect e DSD com motivo visível |
| `TASK-E19-07-AC-3` | um álbum de 10 faixas na biblioteca local com histórico de reprodução carregado (3 reproduções completas da faixa 2 e 1 da faixa 7), o vitrola ligado e o `WearState` serializado em bytes versionados pelo módulo, num diretório de dados temporário que sobrevive ao encerramento do processo | o processo do app é encerrado e reiniciado, o mesmo álbum é tocado de novo, um segundo álbum é aberto em seguida e o comando `Comprar disco novo` é aplicado só ao primeiro álbum; então os bytes dos dois `WearState` são relidos do armazenamento | WearState persistido por álbum |
| `TASK-E19-07-AC-4` | o adaptador do vitrola já integrado no grafo e o corpus de `libs/engine/tests/fixtures/vit/` com senos de 1, 5, 10 e 15 kHz aos níveis −20, −6 e 0 dBFS em 44,1, 48, 96, 192 e 384 kHz, mais `sine-f32le.wav` e `sine-6ch-s24le.wav` copiados de `libs/decode/tests/fixtures/pcm` com o hash conferido contra o `MANIFEST.txt`, semente fixa por cenário e o oráculo de cada teste escrito fora do crate de processamento | `cargo test --release --test vit --locked` no repo `vynamp-engine` executa os dez cenários VIT-T-01..10 do `reference/audio/10-vitrola.md` §7 e grava o relatório por cenário em `tests/out/vit.json` | os testes VIT-T-01..10 rodam com oráculo independente. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd libs/engine && cargo test --release --test vit --locked` sobre `tests/fixtures/vit/`, montado por cópia de `libs/decode/tests/fixtures/pcm/sine-f32le.wav` e `sine-6ch-s24le.wav` (SHA-256 conferido contra o `MANIFEST.txt` daquele diretório) mais `ffmpeg -f lavfi -i sine=frequency=1000:sample_rate=192000:duration=5 -ac 2 -c:a pcm_f32le` para cada par frequência/nível/taxa, e um diretório temporário com dois álbuns e os bytes do `WearState`; o oráculo de AC-1 é a própria entrada sem o nó, o de VIT-T-08 é uma FFT de 4 096 pontos calculada fora do crate de processamento, o de VIT-T-06 são os bytes relidos do disco, o de VIT-T-10 é o LUFS integrado das duas versões medido pelo próprio teste, e a CPU de VIT-T-09 sai de `std::time::Instant` em 10 000 blocos de 512 frames a 96 kHz estéreo | Esperado: VIT-T-01 com diferença máxima 0,0 em f32 e nenhum degrau acima do limiar depois dos 20 ms de crossfade; VIT-T-02 com estado de bypass e o motivo literal de bit-perfect e de DSD nas duas trocas; VIT-T-03 com vinil marcado indisponível e a válvula processando os 6 canais de `sine-6ch-s24le.wav`; VIT-T-04 com o pico de crackle dentro do limite declarado no construtor e o clipping ainda indicado; VIT-T-06 com desgaste preservado e reset só do álbum pedido; VIT-T-08 com os produtos de aliasing abaixo do limiar do módulo, número que ainda não existe e precisa ser fixado na implementação; VIT-T-09 publicado como número de CPU e latência contra a meta de menos de 5% de um núcleo a 96 kHz, que é meta e não gate; VIT-T-10 com a diferença de LUFS dentro da tolerância, também a definir. Hoje nada disso existe: `libs/engine` tem 1 arquivo `.rs` (`src/lib.rs`, 6 linhas, só `pub const VERSION`), `Cargo.toml` com `[dependencies]` vazia e nenhum `tests/`, então `cargo test --all --locked` roda 0 testes em 2 suítes e `--test vit` falha por alvo inexistente; não há nenhum código do vitrola em nativo, `specs/dsp.md` e `specs/engine.md` §5 estão PENDENTES, não há fixture de DSD (o `vynamp-decode` tem 9 crates e nenhum de DSD) nem de álbum com histórico, e `libs/output` só tem `NullHost` e `CaptureHost`, sem backend de OS. |
| Hardware | nenhum: os dez cenários processam blocos em memória e comparam amostras com oráculos offline, sem abrir dispositivo de áudio; a troca para bit-perfect e DoP é exercitada como pedido de política no `NullHost`, não como áudio entregue a um DAC, e este ambiente Windows não tem DAC DSD, placa pro, aparelho Android, iOS nem macOS — a verificação com hardware real é de `TASK-E12-04` e `TASK-E04-05` e fica como limitação declarada, não como aprovação | ausência é limitação registrada, não aprovação |

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
