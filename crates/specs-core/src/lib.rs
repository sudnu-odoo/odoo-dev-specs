//! Core logic of Odoo Dev Specs, independent of the desktop shell:
//! chat events, AI provider adapters and the on-disk session workspace.

pub mod chat;
pub mod claude;
pub mod events;
mod fsutil;
pub mod locate;
pub mod runner;
pub mod transcript;
pub mod workspace;
