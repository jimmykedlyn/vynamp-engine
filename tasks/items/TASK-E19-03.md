# TASK-E19-03 — Dissolver o vitrola no DSP interno do app

> Gerado pelo passo 37 a partir de `tasks/backlog.json`. Os campos de estado vêm do
> backlog e não devem ser editados aqui. Critério de aceite: **PENDENTE** de autoria —
> a coluna `Then` é o requisito aprovado, copiado literal; `Given` e `When` ainda
> não existem.

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

<!-- PENDENTE: o que esta TASK não faz, mesmo parecendo relacionado. -->

## Antes de começar

- [ ] Spec do módulo aprovada e o critério de aceite citado já existe nela
- [ ] Worktree própria criada (`cargo xtask repo worktree task-e19-03-claude-<slug>`)
- [ ] Fixtures/hardware necessários disponíveis ou limitação registrada
- [ ] Nenhuma TASK em execução tocando os mesmos arquivos

## Critérios de aceite

| ID | Given | When | Then |
|---|---|---|---|
| `TASK-E19-03-AC-1` | PENDENTE | PENDENTE | Nenhum documento normativo cita o repositório externo do vitrola |
| `TASK-E19-03-AC-2` | PENDENTE | PENDENTE | a spec descreve módulo DSP interno preservando VIT-T-01..10 |
| `TASK-E19-03-AC-3` | PENDENTE | PENDENTE | o design local não se perde |
| `TASK-E19-03-AC-4` | PENDENTE | PENDENTE | o repositório remoto fica arquivado e reversível. |

## Verificação

| Item | Comando / fixture | Evidência esperada |
|---|---|---|
| Teste automatizado | `cargo test` no repo `vynamp-engine` | verde no CI Gate |
| Oráculo independente | PENDENTE | comparação numérica, não "parece certo" |
| Hardware | PENDENTE | ausência é limitação registrada, não aprovação |

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
