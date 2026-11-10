# TASK-E12-07 — Entregar VST3 e investigar DSP de terceiros

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E12-07` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E12-07` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M5 |
| Prioridade | P2 |
| Estado | planned |
| Depende de | `TASK-E07-10`, `TASK-E04-10` |
| Bloqueia | nenhuma |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 10–25 dias úteis |
| Risco | alto |

## Entrega

Scanner/worker VST3 isolado, compensação de latência e spike WASM DSP separado.

## Boundary

- **Não** implementa a política de circuit breaker nem o lifecycle init/start/event/stop/unload do plugin: `TASK-E07-04` é a dona da regra de três falhas em 60 s, das quotas e dos timeouts, e do estado que aparece no gerenciador. Aqui se entrega o worker VST3 e se prova que, quando o circuit breaker abre, o áudio continua; duplicar a regra aqui daria dois donos ao mesmo limiar.
- **Não** constrói o sandbox de helper nativo nem o consentimento de plugin nativo: `TASK-E07-05` entrega OS sandbox, RPC autenticado, memória compartilhada limitada e a regra de não carregar DLL arbitrária no processo principal. O worker VST3 é uma instância isolada de áudio, não o helper genérico de extensões.
- **Não** prova fuel, epoch interruption nem limite de memória do runtime WASM: `TASK-E07-01` é a dona do spike Wasmtime/WIT e do ADR-004. Esta TASK só impõe a restrição de que nada de WASM rode na callback sem deadline medido, e o mecanismo de interrupção que torna isso verificável vem de lá.
- **Não** soma latência no clock apresentado nem faz a compensação de caminhos paralelos no grafo: `TASK-E04-03` é a dona do grafo compilado, da anotação de latência por nó e do swap em fronteira de bloco. O plugin declara `latency_frames()`; quem integra esse valor ao relógio é o grafo.
- **Não** gera o inventário SPDX, o `THIRD_PARTY_NOTICES` nem o SBOM que passariam a incluir o SDK: `TASK-E11-03` é a dona do inventário de codecs, fontes e assets e `TASK-E11-04` do CI e supply chain. O que esta TASK entrega é o veredito de licença do VST3 SDK, que é a entrada desse inventário, não o documento final.
- **Não** mede CPU e latência do host VST3 na matriz de hardware: `TASK-E04-10` é a dona da matriz numérica e de hardware e dos claims limitados a combinações comprovadas. Aqui a prova é de isolamento e de fonte, não de número publicado por combinação de driver e DAC.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e12-07-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E12-07-AC-1` | dois VST3 de teste compilados do SDK pinado e instalados no diretório de plugins: um que entra em laço infinito dentro de `Processor::process` e outro que aborta no primeiro bloco, com o scanner rodando em processo próprio e o worker de áudio em processo separado do player | o usuário adiciona o plugin que trava à cadeia com o transporte tocando, o worker deixa de responder e o timeout de inicialização declarado no contexto do plugin expira, e em seguida o usuário tenta adicioná-lo de novo pelo gerenciador | Plugin travado bloqueado |
| `TASK-E12-07-AC-2` | uma sessão com `sine-f32le.wav` tocando pelo grafo do engine, o worker VST3 já carregado e medindo a latência que declarou, e a posição apresentada sendo lida por snapshot da callback | o processo do worker é morto com `TerminateProcess` no meio de um bloco, e depois o usuário executa pause, play e seek sem fechar o player | crash não derruba player |
| `TASK-E12-07-AC-3` | o spike de DSP em WASM (`spike/wasm-dsp`) e o grafo do engine com o nó de plugin ligado, mais o guarda estático que delimita as regiões `RT-BEGIN` e `RT-END` nos fontes e o histograma de deadline da callback | roda-se o teste que varre os fontes entre `RT-BEGIN` e `RT-END`, coleta-se o histograma por pelo menos 1M invocações de callback, e tenta-se inserir o nó WASM na cadeia antes de existir um p99.9 medido contra o período negociado | não executar WASM na callback sem prova de deadline |
| `TASK-E12-07-AC-4` | o pacote do VST3 SDK na versão que se vai fixar, com o `LICENSE.txt` e o cabeçalho de `pluginterfaces/` vindos do próprio pacote, e o `LEGAL.md` e `security/02-privacidade-licencas.md` do projeto como referência de teto de licença `MIT OR Apache-2.0` | a revisão registra em `docs/sources/vst3-sdk.md` a licença do header, a condição de redistribuição, o que é exigido do usuário final e o veredito permitido ou proibido, e o texto registrado é confrontado linha a linha com o do pacote baixado | revisar SDK license. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cargo test --package vynamp_engine --test vst3_host --locked` com as fixtures `tests/fixtures/vst3/hang.vst3` (laço em `Processor::process`) e `tests/fixtures/vst3/abort.vst3` (abort no primeiro bloco) compiladas do SDK pinado com `cl.exe`, mais `cargo test --test rt_guard --locked`, que varre `src/` entre os marcadores `RT-BEGIN` e `RT-END` e rejeita tokens de alocação, I/O, log e runtime WASM, e a conferência de `docs/sources/vst3-sdk.md` contra o `LICENSE.txt` do pacote | esperado: o plugin `hang` entra na blacklist com o timeout medido e um motivo legível no gerenciador, o `abort` reanexa o worker em até 1 bloco de callback sem parar a reprodução e o contador de underruns do player fecha o teste em 0, e a varredura `RT-BEGIN`/`RT-END` não acha `wasm` nem `Wasmtime`. Hoje, medido em 2026-11-10, `libs/engine/src/lib.rs` tem 6 linhas e só `pub const VERSION`, `cargo test --all --locked` roda 0 testes em 2 suítes, não existe `tests/` nem `docs/sources/`, o SDK VST3 não está nesta máquina (sem `C:\VST3_SDK` e sem `C:\Program Files\Common Files\VST3`), e o toolchain C++ está fora do PATH: `cl.exe` 14.36.32532 só existe em `C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC\14.36.32532\bin\Hostx64\x64`, `cmake.exe` 3.26.0-msvc3 só dentro do pacote de CMake do Visual Studio e o LLVM do VS traz apenas `clang-format` e `clang-tidy`, sem `clang.exe`. |
| Hardware | nenhum: a prova é de isolamento de processo, de fonte e de código de retorno, e os fixtures de plugin são compilados do SDK com o `cl.exe` presente, sem depender de DAC profissional. O que falta neste ambiente é o SDK VST3 em si e qualquer plugin VST3 instalado, além do DAC USB 24/192 e da interface 44,1/48 que `quality/01-estrategia-testes.md` exige para a matriz de hardware; há apenas áudio Intel e Conexant integrado. A medição de latência e estabilidade por dispositivo continua sendo de `TASK-E04-10`. | ausência é limitação registrada, não aprovação |

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
