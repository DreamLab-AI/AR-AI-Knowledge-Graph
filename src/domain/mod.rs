//! Storage-agnostic domain kernels.
//!
//! Pure aggregate/value-object logic with no transport or persistence
//! dependency. Adapters (SQLite, Oxigraph, the ACSP producer, the REST
//! handlers) sit outside this module and depend inward on it.

pub mod broker;
