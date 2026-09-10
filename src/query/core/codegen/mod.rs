//! Backend-neutral SQL generation.

mod batch;
mod context;
mod entry;
mod expression;
mod orchestrate;
mod params;
mod select;
mod sources;
mod virtual_tables;

pub(crate) use entry::{
    compile_batch, compile_presentation_lookup, compile_query, prepare_query, prepare_query_with,
};

#[cfg(test)]
mod tests;
