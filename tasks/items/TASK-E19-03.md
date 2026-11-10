# TASK-E19-03 — Dissolver o vitrola no DSP interno do app

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. A coluna `Then` é o requisito aprovado,
> copiado literal do backlog; `Given` e `When` foram autorados na execução.

| Campo | Valor |
|---|---|
| ID | `TASK-E19-03` |
| Módulo dono | `vynamp-engine` |
| Repo | `jimmykedlyn/vynamp-engine` |
| REQ coberto | `REQ-E19-03` |
| Critérios da spec | pendente — `specs/engine.md` §5 |
| Fase | M3 |
| Prioridade | P0 |
| Estado | planned |
| Depende de | `TASK-E04-03` |
| Bloqueia | `TASK-E19-07` |
| Modelo executor | Opus — áudio/RT, concorrência, segurança ou release |
| Esforço | 1–3 dias úteis |
| Risco | médio |

## Entrega

Design aprovado do vitrola incorporado como módulo DSP interno, referências ao repositório externo removidas e repositório vitrola arquivado de forma reversível.

## Boundary

- **Não** implementa o vinil nem a válvula: nenhuma linha de Rust, nenhum trait `Processor`, nenhum coeficiente. A implementação e a integração no grafo do player são `TASK-E19-07`; aqui apenas se reescreve a embalagem documental do design, e a confusão é plausível porque a AC-2 fala em `preservar VIT-T-01..10`, que são testes de um módulo que ainda não existe.
- **Não** decide a posição do módulo na cadeia nem fecha o conflito D-DSP-05 entre o pedido de ficar depois do resampler e o resampler final único do contrato 07. O grafo e a anotação de latência são `TASK-E04-03` e a integração é `TASK-E19-07`; o §3 da spec mantém o conflito aberto e esta TASK não pode alterá-lo, o que é fácil de esquecer porque §3 é justamente a seção que se revisa ao dissolver a biblioteca.
- **Não** arquiva, abre ao público nem protege nenhum dos outros repositórios do conjunto: os sete repositórios públicos e o `CI Gate` exigido em `develop` são `TASK-E19-06`. A confusão é plausível porque a AC-4 pede arquivamento reversível, a mesma família de operação de ciclo de vida que a `TASK-E19-06` aplica em escala.
- **Não** muda o CI do umbrella nem remove pin de action: workflows, matriz de alvos e contexto `CI Gate` são `TASK-E19-01`, e README, `SECURITY.md` e governança do repositório principal são `TASK-E19-02`. Parece do escopo porque dissolver a biblioteca externa implica varrer citações, e o primeiro lugar onde se pensa em procurar citação é o YAML do CI.
- **Não** cria repositório próprio para a spec do motor nem a move para `libs/engine/specs/`: a fonte normativa continua no umbrella em `reference/audio/10-vitrola.md`, e re-apontar specs para repositórios de módulo é `TASK-E19-04`, que trata de Restore e Converter. A confusão é plausível porque a TASK pertence ao módulo `vynamp-engine` e o instinto é levar a spec para dentro do repositório dono.

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e19-03-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E19-03-AC-1` | o repositório umbrella no commit `7d330d6`, com o conjunto normativo fechado em `reference/`, `adr/`, `design/`, `product/`, `quality/`, `delivery/`, `architecture/`, `security/`, `tasks/` e `config/`, mais `AGENTS.md` e `INDEX.md`, e os registros de origem separados em `evidence/PR/task-e19-03/` e `_arquivo/` | `rg -n 'jimmykedlyn/vitrola'` roda sobre os dez diretórios normativos e os dois arquivos de raiz e, numa segunda passada, sobre `evidence/` e `_arquivo/`, e as ocorrências de cada passada são listadas por caminho e linha e cada uma recebe a marca de registro de origem ou de citação normativa pendente | Nenhum documento normativo cita o repositório externo do vitrola |
| `TASK-E19-03-AC-2` | `reference/audio/10-vitrola.md` na versão 0.8.1, 205 linhas, com §1 declarando vinil e válvula módulos internos sem biblioteca nem repositório, §7 com 10 linhas de tabela de `VIT-T-01` a `VIT-T-10` e §8 com as 9 subseções `8.1` a `8.9` do design incorporado em 2026-11-05 | o documento é lido inteiro e cada uma das 10 linhas de `VIT-T-01` a `VIT-T-10` da §7 é confrontada com o objetivo, o método e o critério homônimos do texto do design em §8, e com as frases de §1 e §3 que afirmam módulo interno do app | a spec descreve módulo DSP interno preservando VIT-T-01..10 |
| `TASK-E19-03-AC-3` | o clone local em `D:\Projects\vitrola` na branch `develop` em `75c4ca2`, um commit à frente de `origin` em `3d44c68`, e a cópia preservada em `evidence/PR/task-e19-03/vitrola-design.md` com 88 linhas, sendo 11 de cabeçalho e 77 de corpo | o corpo da cópia é extraído a partir do primeiro cabeçalho `# vitrola` e comparado byte a byte com `git -C D:\Projects\vitrola show 75c4ca2:docs/superpowers/specs/2026-10-23-vitrola-design.md`, e as ocorrências de `agulha` do commit publicado `3d44c68` são procuradas no mesmo corpo | o design local não se perde |
| `TASK-E19-03-AC-4` | o repositório `jimmykedlyn/vitrola` com `origin` em `https://github.com/jimmykedlyn/vitrola.git`, branch padrão `main`, `pushedAt` `2026-10-23T16:14:16Z`, já com `isArchived` verdadeiro e visibilidade pública, e o clone local segurando `75c4ca2` ainda não publicado | o mantenedor exercita o caminho de volta com `gh repo unarchive jimmykedlyn/vitrola`, confere `gh repo view jimmykedlyn/vitrola --json isArchived` e reaplica `gh repo archive jimmykedlyn/vitrola`, registrando o par de saídas e o resultado de cada passo | o repositório remoto fica arquivado e reversível. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | `cd D:\Projects\vynamp` e então (1) `rg -n 'jimmykedlyn/vitrola' reference/ adr/ design/ product/ quality/ delivery/ architecture/ security/ tasks/ config/ AGENTS.md INDEX.md`, (2) `rg -c 'VIT-T-[0-9][0-9]' reference/audio/10-vitrola.md` e `rg -c '^### 8[.]' reference/audio/10-vitrola.md`, (3) comparação byte a byte, em `python -I`, do corpo de `evidence/PR/task-e19-03/vitrola-design.md` a partir do primeiro `# vitrola` com `git -C D:\Projects\vitrola show 75c4ca2:docs/superpowers/specs/2026-10-23-vitrola-design.md`, (4) `gh repo view jimmykedlyn/vitrola --json isArchived,visibility` | Medido hoje, 2026-11-10: (1) 0 ocorrências nos doze alvos normativos, saída vazia e código de retorno 1 — as 13 ocorrências de `jimmykedlyn/vitrola` do repositório estão todas fora deles, 2 em `evidence/PR/task-e19-03/vitrola-design.md`, 3 em `evidence/PR/task-e19-03/README.md`, 3 em `evidence/PR/task-e19-03/pr-body.md` e 5 em `_arquivo/repo/tasks/items/TASK-E19-03.md`, todas registro de origem; (2) 12 ocorrências de `VIT-T-nn` no arquivo, sendo as 10 da tabela da §7 nas linhas 113 a 122 e 2 menções fora dela, mais 9 cabeçalhos `### 8.`, batendo com a AC-2; (3) corpo idêntico, 8019 caracteres e 8325 bytes em UTF-8, 77 linhas nos dois lados, e 3 ocorrências de `agulha` no texto preservado contra 6 no commit publicado `3d44c68`, que é a razão de a cópia vir de `75c4ca2`; (4) `isArchived=true` e `visibility=PUBLIC`, ou seja, o arquivamento já está aplicado e falta apenas que alguém exercite `gh repo unarchive`, que não foi rodado. O que não existe: nenhum gate verifica citação de repositório externo — `cargo xtask check-docs` passa com 210 arquivos Markdown, 199 tarefas, 197 requisitos e `spec_version` 0.7.0, e `cargo xtask check-tasks` passa com 199 tarefas, mas os dois não procuram a string `jimmykedlyn/vitrola`, então o verde deles não prova a AC-1; não há registro de reversibilidade (um `gh repo unarchive` executado e registrado); e `D:\Projects\vitrola` continua no disco como única cópia de `75c4ca2`, com a branch 1 commit à frente de `origin` e nada empurrado. |
| Hardware | nenhum: a prova é leitura de arquivos versionados, comparação byte a byte e chamada de `gh repo view` autenticada como jimmykedlyn — não se renderiza um único frame de áudio, não se executa `cargo test` de DSP e nenhum dispositivo de áudio é aberto; `gh` respondendo é rede, não hardware de áudio, e as plataformas sujeitas a hardware (Android, iOS, macOS, ASIO, CoreAudio, WASAPI exclusivo) só entram em `TASK-E19-07` e `TASK-E04-05` | ausência é limitação registrada, não aprovação |

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
