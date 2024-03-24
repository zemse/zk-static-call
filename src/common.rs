use axiom_codec::HiLo;
use axiom_eth::halo2_base::{AssignedValue, Context};
use eth_types::Field;
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, Value},
    plonk::Assigned,
};
use zkevm_circuits::witness::Rw;

use crate::init_state::InitStateTable;

pub type AxiomAssignedValue<F> = AssignedValue<F>;
pub type Halo2AssignedCell<F> = AssignedCell<Assigned<F>, F>;

// Note: Assign trait is used because AnySubqueryResult is foreign type
pub trait Payload<F: Field, A, H> {
    fn from_rw(rw: &Rw, block_number: u64) -> Option<Self>
    where
        Self: std::marker::Sized;

    fn assign_axiom(&self, ctx: &mut Context<F>) -> A;

    fn assign_halo2(
        &self,
        config: &InitStateTable<F>,
        layouter: &mut impl Layouter<F>,
        randomness: Value<F>,
    ) -> H;
}

#[derive(Clone, Default, Debug)]
pub struct HiLoRlc<T> {
    pub hilo: HiLo<T>,
    pub rlc: T,
}

impl<T> HiLoRlc<T> {
    pub fn from(hi: T, lo: T, rlc: T) -> Self {
        Self {
            hilo: HiLo::from_hi_lo([hi, lo]),
            rlc,
        }
    }

    pub fn hi(&self) -> T
    where
        T: Copy,
    {
        self.hilo.hi()
    }

    pub fn lo(&self) -> T
    where
        T: Copy,
    {
        self.hilo.lo()
    }

    pub fn rlc(&self) -> T
    where
        T: Copy,
    {
        self.rlc
    }
}
