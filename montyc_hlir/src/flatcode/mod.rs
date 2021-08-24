//! A post-processed, flattened, AST, with extra semantics sprinkled in.
//!
//! Typically generated immedietly after parsing and passed around to verious
//! semantic passes, i.e. static type checking or to the const interpreter.
//!
#![allow(missing_docs)]

mod lower;
pub mod raw_inst;

use std::fmt::Display;

use montyc_core::SpanRef;

use crate::flatcode::raw_inst::Const;

use self::raw_inst::RawInst;

const INVALID_VALUE: usize = std::usize::MAX;

/// associated attributes of an instruction.
#[derive(Debug, Clone, Default)]
pub struct InstAttrs {
    span: Option<SpanRef>,
}

/// An instruction in a sequence of code and it's output value.
#[derive(Debug, Clone)]
pub struct FlatInst<V = usize, R = SpanRef> {
    pub op: RawInst<V, R>,
    pub value: V,
    pub attrs: InstAttrs,
}

#[allow(missing_docs)]
#[derive(Debug)]
pub enum SequenceType {
    Module,
    // Class,
    Function,
}

/// A sequence of flatcode.
#[derive(Debug)]
pub struct FlatSeq {
    pub(crate) inst: Vec<FlatInst>,
    kind: SequenceType,
}

impl FlatSeq {
    pub fn inst(&self) -> &[FlatInst] {
        self.inst.as_slice()
    }
}

/// An SSA-based, linear, sequence of code-like IR generated by flattening an AST.
#[derive(Debug, Default)]
pub struct FlatCode {
    sequence_index: usize,
    pub(crate) sequences: Vec<FlatSeq>,
}

impl Display for FlatCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (s, sequence) in self.sequences.iter().enumerate() {
            write!(f, "sequence({}):\n", s)?;

            for inst in sequence.inst.iter() {
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
            sequences: vec![FlatSeq {
                inst: vec![],
                kind: SequenceType::Module,
            }],
        }
    }

    /// The sequences of the code.
    pub fn sequences(&self) -> &[FlatSeq] {
        &self.sequences
    }

    /// Check if the given sequence only has an ellipsis instruction inside it.
    pub fn is_sequence_ellipsis_stubbed(&self, seq: usize) -> bool {
        self.sequences
            .get(seq)
            .map(|seq| {
                matches!(
                    seq.inst.as_slice(),
                    [FlatInst {
                        op: RawInst::Const(Const::Ellipsis),
                        ..
                    }]
                )
            })
            .unwrap_or(false)
    }

    fn inst(&mut self, raw_inst: RawInst) -> usize {
        match self.sequences.get_mut(self.sequence_index) {
            Some(FlatSeq { inst: seq, .. }) => {
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

    fn with_new_sequence(
        &mut self,
        size_hint: usize,
        kind: SequenceType,
        f: impl Fn(&mut Self),
    ) -> usize {
        let old_index = self.sequence_index;

        self.sequence_index = self.sequences.len();
        self.sequences.push(FlatSeq {
            inst: Vec::with_capacity(size_hint),
            kind,
        });

        let index = self.sequence_index;

        f(self);

        self.sequence_index = old_index;
        index
    }
}
