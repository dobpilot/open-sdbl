//! Backend-neutral SQL generation.

mod accounting;
mod batch;
mod constants;
mod context;
mod entry;
mod expression;
mod nested;
mod orchestrate;
mod params;
mod select;
mod separators;
mod sources;
mod totals;
mod virtual_tables;
mod windowed;

pub(crate) use entry::{
    compile_batch, compile_presentation_lookup, compile_query, prepare_query, prepare_query_with,
};

#[cfg(test)]
mod tests;
