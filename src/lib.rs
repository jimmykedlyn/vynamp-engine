//! vynamp-engine
//!
//! Específico deste módulo: ver [specs/](specs/) no próprio repo —
//! [`specs/engine.md`](specs/engine.md) §3.2–§3.4 e §3.7 descrevem os tipos de
//! domínio que este crate publica. §3 é rascunho; só §5 foi aprovada, em
//! 2026-11-18.
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
//! O lado que apresenta compara a geração de cada buffer com
//! `PresentationGate::admits` (load atômico, sem lock nem alocação) e descarta
//! a geração velha. O ponto de linearização é esse load: um período que já foi
//! admitido antes de um `Seek` ou `Stop` termina de tocar; nenhum buffer da
//! geração velha é admitido depois. `Engine::publish_buffer` também compara sem
//! lock, mas é só filtro antecipado: um buffer pode ficar velho entre a
//! comparação e o `push`, e quem o descarta é o portão.
//!
//! Cancelamento é consultado por polling cooperativo nos workers. O lock de
//! estado do `Engine` só é tomado por comandos, gatilhos e leituras de
//! diagnóstico, nunca por `publish_buffer` nem pelo portão.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Versão do módulo. O contrato de release do umbrella fixa a tag.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Domínio de reprodução: estados, comandos, eventos, geração e cancelamento.
pub mod playback;

/// Pipeline de áudio: ring SPSC, dreno de EOF, relógio de frames apresentados e
/// transições entre fontes (TASK-E01-05).
pub mod pipeline;
