//! vynamp-engine
//!
//! Específico deste módulo: ver [specs/](specs/) no próprio repo —
//! [`specs/engine.md`](specs/engine.md) §3.2–§3.4 (aprovada 2026-11-18) fixa os
//! tipos de domínio que este crate publica.
//!
//! Este crate integra decode, dsp, output e encode num grafo de reprodução com
//! estado, comandos e eventos. Nenhuma amostra atravessa esta API: primitivos
//! entram, primitivos saem (AGENTS.md §1.3).
//!
//! # Erro
//!
//! Erro é dado, não `panic!`: toda operação pública devolve [`Result`] e o
//! domínio não usa `unwrap`/`expect` em caminho de produção.
//!
//! # Real time
//!
//! A callback de áudio nunca bloqueia: compara a geração do buffer e descarta.
//! Cancelamento é consultado por polling cooperativo nos workers, nunca por
//! lock dentro de caminho de áudio.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Versão do módulo. O contrato de release do umbrella fixa a tag.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Domínio de reprodução: estados, comandos, eventos, geração e cancelamento.
pub mod playback;
