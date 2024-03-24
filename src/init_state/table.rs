use super::{account::Halo2AccountPayload, storage::Halo2StoragePayload, InitState};
use crate::common::{HiLoRlc, Payload};
use eth_types::Field;
use halo2_proofs::{
    circuit::{Layouter, Value},
    plonk::{Advice, Any, Column, ConstraintSystem},
};
use std::marker::PhantomData;
use zkevm_circuits::table::LookupTable;

#[derive(Clone)]
pub struct InitStateTable<F> {
    pub block_number: Column<Advice>, // TODO remove this
    pub address: Column<Advice>,
    pub field_tag: Column<Advice>,
    pub storage_key: HiLoRlc<Column<Advice>>,
    pub value: HiLoRlc<Column<Advice>>,
    pub _marker: PhantomData<F>,
}

impl<F: Field> LookupTable<F> for InitStateTable<F> {
    fn columns(&self) -> Vec<Column<Any>> {
        vec![
            self.address.into(),
            self.field_tag.into(),
            self.storage_key.hi().into(),
            self.storage_key.lo().into(),
            self.storage_key.rlc().into(),
            self.value.hi().into(),
            self.value.lo().into(),
            self.value.rlc().into(),
        ]
    }

    fn annotations(&self) -> Vec<String> {
        vec![
            "address".to_string(),
            "field_tag".to_string(),
            "storage_key_hi".to_string(),
            "storage_key_lo".to_string(),
            "storage_key_rlc".to_string(),
            "value_hi".to_string(),
            "value_lo".to_string(),
            "value_rlc".to_string(),
        ]
    }
}

impl<F: Field> InitStateTable<F> {
    /// Construct a new InitStateTable
    pub fn construct(meta: &mut ConstraintSystem<F>) -> Self {
        Self {
            block_number: meta.advice_column(),
            address: meta.advice_column(),
            field_tag: meta.advice_column(),
            storage_key: HiLoRlc::from(
                meta.advice_column(),
                meta.advice_column(),
                meta.advice_column(),
            ),
            value: HiLoRlc::from(
                meta.advice_column(),
                meta.advice_column(),
                meta.advice_column(),
            ),
            _marker: PhantomData,
        }
    }

    /// Assign the InitStateTable using Layouter
    pub fn load(
        &self,
        input: &InitState<F>,
        layouter: &mut impl Layouter<F>,
        randomness: Value<F>,
    ) -> (Vec<Halo2AccountPayload<F>>, Vec<Halo2StoragePayload<F>>) {
        // TODO get rid of duplicates
        let mut account_payloads = vec![];
        for account in input.accounts.iter() {
            account_payloads.push(account.assign_halo2(self, layouter, randomness));
        }
        let mut storage_payloads = vec![];
        for storage in input.storages.iter() {
            storage_payloads.push(storage.assign_halo2(self, layouter, randomness));
        }
        (account_payloads, storage_payloads)
    }
}
