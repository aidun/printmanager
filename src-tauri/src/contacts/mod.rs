//! Kontaktquellen (nur lesen): Apple Kontakte am Mac; Google und CardDAV folgen (docs/PLAN-Anbindungen.md, K0–K2).

#[cfg(target_os = "macos")]
pub mod apple;
