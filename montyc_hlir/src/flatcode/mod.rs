//! A post-processed, flattened, AST, with extra semantics sprinkled in.
//!
//! Typically generated immedietly after parsing and passed around to verious
//! semantic passes, i.e. static type checking or to the const interpreter.
//!

mod lower;
pub mod raw_inst;

use std::fmt::Display;

use montyc_core::SpanRef;

use self::raw_inst::RawInst;

const INVALID_VALUE: usize = std::usize::MAX;

/// associated attributes of an instruction.
#[derive(Debug, Clone, Default)]
pub struct InstAttrs {
    span: Option<SpanRef>,
    
}

/// An instruction in a sequence of code and it's output value.
#[derive(Debug, Clone)]
pub(crate) struct FlatInst<V = usize, R = SpanRef> {
    pub(crate) op: RawInst<V, R>,
    pub(crate) value: V,
    pub(crate) attrs: InstAttrs,
}

/// An SSA-based, linear, sequence of code-like IR generated by flattening an AST.
#[derive(Debug)]
pub struct FlatCode {
    sequence_index: usize,
    pub(crate) sequences: Vec<Vec<FlatInst>>,
}

impl Display for FlatCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (s, sequence) in self.sequences.iter().enumerate() {
            write!(f, "sequence({}):\n", s)?;

            for inst in sequence.iter() {
                write!(f, "  %{} = {}\n", inst.value, inst.op)?;
            }
        }

        Ok(())
    }
}

impl FlatCode {
    /// Creat a new flatcode builder.
    pub fn new() -> Self {
        Self {
            sequence_index: 0,
            sequences: vec![vec![]],
        }
    }
}

impl FlatCode {
    fn inst(&mut self, raw_inst: RawInst) -> usize {
        match self.sequences.get_mut(self.sequence_index) {
            Some(seq) => {
                seq.push(FlatInst {
                    op: raw_inst,
                    value: seq.len(),
                    attrs: InstAttrs::default(),
                });

                seq.len().saturating_sub(1)
            }

            None => unreachable!(),
        }
    }

    fn with_new_sequence(&mut self, f: impl Fn(&mut Self)) -> usize {
        let old_index = self.sequence_index;

        self.sequence_index = self.sequences.len();
        self.sequences.push(vec![]);

        let index = self.sequence_index;

        f(self);

        self.sequence_index = old_index;
        index
    }
}
