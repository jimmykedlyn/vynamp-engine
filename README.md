# vynamp-engine

**English** · [Português](#português--vynamp-engine)

Integration, domain and the DSP chain, in Rust, `std` only, with no crates.io
dependencies. The only crate that knows `vynamp-decode`, `vynamp-encode`,
`vynamp-output` and `vynamp-dsp` at once.

> **Status: specification.** This library is not implemented. The repository holds the
> module specification, its task list and the tooling both are built with. It ships no
> running code and makes no measured claim.

Licence: `MIT OR Apache-2.0` — see [LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
Security: [GitHub Security Advisories](https://github.com/jimmykedlyn/vynamp-engine/security/advisories/new)
— see [SECURITY.md](SECURITY.md).
Contributing: [CONTRIBUTING.md](CONTRIBUTING.md).

## Scope

Integrates. It owns the queue, the DSP chain, the clock and the translation of
domain types between the libraries it drives. It has no UI, does not open an
audio device on its own and does not encode a file by itself — the applications
and `vynamp-output` do that.

## Builds on its own

```bash
cargo test
```

The agent contract is the umbrella's:
[AGENTS.md](https://github.com/jimmykedlyn/vynamp/blob/develop/AGENTS.md).

# Português — vynamp-engine

[English](#vynamp-engine) · **Português**

Integração, domínio e cadeia DSP, em Rust, só `std`, sem dependências do
crates.io. O único crate que conhece `vynamp-decode`, `vynamp-encode`,
`vynamp-output` e `vynamp-dsp` ao mesmo tempo.

> **Estado: especificação.** Esta biblioteca não está implementada. O repositório contém
> a especificação do módulo, a lista de tarefas dele e o tooling com que os dois são
> construídos. Não entrega código em execução e não afirma resultado medido.

Licença: `MIT OR Apache-2.0` — ver [LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT) e
[LICENSE-APACHE](LICENSE-APACHE).
Segurança: [GitHub Security Advisories](https://github.com/jimmykedlyn/vynamp-engine/security/advisories/new)
— ver [SECURITY.md](SECURITY.md).
Contribuições: [CONTRIBUTING.md](CONTRIBUTING.md).

## Escopo

Integra. É dono da fila, da cadeia DSP, do clock e da tradução de tipos de
domínio entre as bibliotecas que dirige. Não tem UI, não abre dispositivo de
áudio sozinho e não codifica arquivo sozinho — isso é das aplicações e do
`vynamp-output`.

## Compila sozinho

```bash
cargo test
```

O contrato do agente é o do umbrella:
[AGENTS.md](https://github.com/jimmykedlyn/vynamp/blob/develop/AGENTS.md).
