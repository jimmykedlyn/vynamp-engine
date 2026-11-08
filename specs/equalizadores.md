# Spec — vynamp-engine · equalizadores

| Campo | Valor |
|---|---|
| Módulo | `vynamp-engine` |
| Crate(s) | a definir na autoria |
| Tipo | lib |
| Versão da spec | 0.1.0 |
| Data | 2026-11-08 |
| Estado | esqueleto — §3 a §10 pendentes de autoria |
| Dono | a definir |
| Depende de | decode, encode, output, dsp (as quatro) |
| É dependido por | player, converter, restore |

> **Esqueleto gerado pelo passo 37.** §1, §2 e §7 foram decididos em
> `01-arvore-final.md` §2 e valem como norma. §3 a §6 e §8 a §10 exigem projeto de
> API e **não** foram escritos: inventá-los aqui seria fabricar a autoridade que o SDD
> exige. Estado só passa a `aprovada` com §5 preenchida.

## 1. Propósito

Integrar decode, dsp, output e encode num grafo de reproducao com estado, comandos e grafo DSP.

**Não faz:** Nao tem UI, nao gerencia biblioteca, nao abre dispositivo por conta propria sem o output.

## 2. Fronteira

| Permitido | Proibido |
|---|---|
| herda a fronteira de `specs/engine.md` | herda as proibicoes de `specs/engine.md` |

## 3. API pública

PENDENTE — projeto de API. Só o que estiver aqui é contrato.

## 4. Requisitos

PENDENTE — cada requisito com ID estável `ENGINE-REQ-nn` e REQ de produto,
rastreável até uma TASK e um teste.

## 5. Critérios de aceite

PENDENTE — Given/When/Then, coerente de fora. Enquanto esta seção estiver vazia,
nenhuma TASK deste módulo pode sair de `planned`.

## 6. Erros e cancelamento

PENDENTE — erro é dado, não `panic!`. Cancelamento é explícito, não timeout.

## 7. Fora de escopo

Vinyl/Tube e VST3 entram como modulos internos deste grafo (E19-07, E12-07). Jobs persistidos (E08-01) entram como infraestrutura nao-audio, decisao registrada

## 8. Dependências

Depende de: decode, encode, output, dsp (as quatro)

É dependido por: player, converter, restore

## 9. Testes e fixtures

PENDENTE — oráculo numérico onde couber. Teste que depende de arquivo grande vive
atrás de `#[ignore]` com motivo escrito e variável de ambiente documentada.

## 10. Limitações conhecidas

PENDENTE — limitação declarada não reprova a entrega; limitação omitida reprova.
