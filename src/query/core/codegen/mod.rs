//! Backend-neutral SQL generation.

mod context;
mod entry;
mod expression;
mod orchestrate;
mod select;
mod sources;
mod virtual_tables;

pub(crate) use entry::{compile_presentation_lookup, compile_query, prepare_query};

#[cfg(test)]
mod tests;
