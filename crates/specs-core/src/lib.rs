//! Core logic of Odoo Dev Specs, independent of the desktop shell:
//! chat events, AI provider adapters and the on-disk session workspace.

pub mod antigravity;
pub mod attachments;
pub mod chat;
pub mod claude;
pub mod events;
mod fsutil;
pub mod locate;
pub mod models;
pub mod providers;
pub mod runner;
pub mod settings;
pub mod transcript;
pub mod turn;
pub mod workspace;
