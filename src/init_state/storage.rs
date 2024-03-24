use crate::{
    common::{AxiomAssignedValue, Halo2AssignedCell, HiLoRlc, Payload},
    init_state::InitStateTable,
};
use axiom_codec::{
    types::{field_elements::AnySubqueryResult, native::StorageSubquery},
    HiLo,
};
use axiom_eth::halo2_base::Context;
use eth_types::{BigEndianHash, Field, ToScalar, H256};
use halo2_proofs::{
    circuit::{Layouter, Value},
    plonk::Assigned,
};
use std::marker::PhantomData;
use zkevm_circuits::evm_circuit::util::rlc;

pub struct StoragePayload<AssignedType> {
    pub block_number: AssignedType,
    pub address: AssignedType,
    pub slot: HiLoRlc<AssignedType>,
    pub value: HiLoRlc<AssignedType>,
}

pub type StorageSubqueryResult<F> = (AnySubqueryResult<StorageSubquery, H256>, PhantomData<F>);
pub type AxiomStoragePayload<F> = StoragePayload<AxiomAssignedValue<F>>;
pub type Halo2StoragePayload<F> = StoragePayload<Halo2AssignedCell<F>>;

// Note: Assign trait is used because AnySubqueryResult is foreign type
impl<F: Field> Payload<F, AxiomStoragePayload<F>, Halo2StoragePayload<F>>
    for StorageSubqueryResult<F>
{
    fn from_rw(rw: &zkevm_circuits::witness::Rw, block_number: u64) -> Option<Self> {
        Some((
            AnySubqueryResult {
                subquery: StorageSubquery {
                    block_number: block_number as u32,
                    addr: rw.address().unwrap(),
                    slot: rw.storage_key().unwrap(),
                },
                value: H256::from_uint(&rw.value_word()),
            },
            PhantomData,
        ))
    }

    fn assign_axiom(&self, ctx: &mut Context<F>) -> AxiomStoragePayload<F> {
        AxiomStoragePayload {
            block_number: ctx.load_witness(F::from(self.0.subquery.block_number as u64)),
            address: ctx.load_witness(self.0.subquery.addr.to_scalar().unwrap()),
            slot: HiLoRlc {
                hilo: HiLo::<F>::from(H256::from_uint(&self.0.subquery.slot)).assign(ctx),
                rlc: ctx.load_witness(F::ZERO), // TODO set correct value
            },
            value: HiLoRlc {
                hilo: HiLo::<F>::from(self.0.value).assign(ctx),
                rlc: ctx.load_witness(F::ZERO), // TODO set correct value
            },
        }
    }

    fn assign_halo2(
        &self,
        config: &InitStateTable<F>,
        layouter: &mut impl Layouter<F>,
        randomness: Value<F>,
    ) -> Halo2StoragePayload<F> {
        layouter
            .assign_region(
                || "myregion",
                |mut region| {
                    let offset = 0;

                    let address: F = self.0.subquery.addr.to_scalar().unwrap();

                    let key = H256::from_uint(&self.0.subquery.slot);
                    let key_rlc = randomness.map(|r| rlc::value(key.as_bytes(), r));
                    let key_hilo = HiLo::<F>::from(key);

                    let value_rlc = randomness
                        .map(|r| Assigned::Trivial(rlc::value(self.0.value.as_bytes(), r)));
                    let value_hilo = HiLo::<F>::from(self.0.value);

                    let assigned_block_number = region.assign_advice(
                        || format!("is table row {offset} block_number"),
                        config.block_number,
                        offset,
                        || {
                            Value::known(Assigned::Trivial(F::from(
                                self.0.subquery.block_number as u64,
                            )))
                        },
                    )?;
                    let assigned_address = region.assign_advice(
                        || format!("is table row {offset} address"),
                        config.address,
                        offset,
                        || Value::known(Assigned::Trivial(address)),
                    )?;

                    let assigned_storage_key_hi = region.assign_advice(
                        || format!("is table row {offset} storage_key hi"),
                        config.storage_key.hi(),
                        offset,
                        || Value::known(Assigned::Trivial(key_hilo.hi())),
                    )?;
                    let assigned_storage_key_lo = region.assign_advice(
                        || format!("is table row {offset} storage_key lo"),
                        config.storage_key.lo(),
                        offset,
                        || Value::known(Assigned::Trivial(key_hilo.lo())),
                    )?;
                    // TODO add gate which ensures rlc == hilo
                    let assigned_storage_key_rlc = region.assign_advice(
                        || format!("is table row {offset} storage_key rlc"),
                        config.storage_key.rlc(),
                        offset,
                        || key_rlc.map(Assigned::Trivial),
                    )?;

                    let assigned_value_hi = region.assign_advice(
                        || format!("is table row {offset} value hi"),
                        config.value.hi(),
                        offset,
                        || Value::known(Assigned::Trivial(value_hilo.hi())),
                    )?;
                    let assigned_value_lo = region.assign_advice(
                        || format!("is table row {offset} value lo"),
                        config.value.lo(),
                        offset,
                        || Value::known(Assigned::Trivial(value_hilo.lo())),
                    )?;
                    // TODO add gate which ensures rlc == hilo
                    let assigned_value_rlc = region.assign_advice(
                        || format!("is table row {offset} value rlc"),
                        config.value.rlc(),
                        offset,
                        || value_rlc,
                    )?;
                    // offset += 1;

                    Ok(Halo2StoragePayload {
                        block_number: assigned_block_number,
                        address: assigned_address,
                        slot: HiLoRlc::from(
                            assigned_storage_key_hi,
                            assigned_storage_key_lo,
                            assigned_storage_key_rlc,
                        ),
                        value: HiLoRlc::from(
                            assigned_value_hi,
                            assigned_value_lo,
                            assigned_value_rlc,
                        ),
                    })
                },
            )
            .unwrap()
    }
}
