use super::table::InitStateTable;
use crate::common::{AxiomAssignedValue, Halo2AssignedCell, HiLoRlc, Payload};
use axiom_codec::{
    types::{field_elements::AnySubqueryResult, native::AccountSubquery},
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

// | Account State Field     | Max bytes   |
// |-------------------------|-------------|
// | nonce                   | ≤8          |
// | balance                 | ≤12         |
// | storageRoot             | 32          |
// | codeHash                | 32          |

#[derive(Clone, Copy)]
pub enum AccountFieldIdx {
    Nonce = 0,
    Balance = 1,
    // StorageRoot is not queried in the Init State
    #[allow(dead_code)]
    StorageRoot = 2,
    CodeHash = 3,
}

impl AccountFieldIdx {
    pub fn from_field_tag(value: Option<u64>) -> Option<Self> {
        match value.unwrap() {
            /* AccountFieldTag::Nonce */ 0 => Self::Nonce.into(),
            /* AccountFieldTag::Balance */ 1 => Self::Balance.into(),
            /* AccountFieldTag::KeccakCodeHash */ 2 => Self::CodeHash.into(),
            /* AccountFieldTag::CodeHash */ 3 => Self::CodeHash.into(),
            /* AccountFieldTag::CodeSize */ 4 => Self::CodeHash.into(),
            /* AccountFieldTag::NonExisting */ 5 => None,
            _ => None,
        }
    }
}

pub struct AccountPayload<AssignedType> {
    pub block_number: AssignedType, // TODO constrain the block number later
    pub address: AssignedType,
    pub field_idx: AssignedType,
    pub value: HiLoRlc<AssignedType>,
}

pub type AccountSubqueryResult<F> = (AnySubqueryResult<AccountSubquery, H256>, PhantomData<F>);
pub type AxiomAccountPayload<F> = AccountPayload<AxiomAssignedValue<F>>;
pub type Halo2AccountPayload<F> = AccountPayload<Halo2AssignedCell<F>>;

// Note: Assign trait is used because AnySubqueryResult is foreign type
impl<F: Field> Payload<F, AxiomAccountPayload<F>, Halo2AccountPayload<F>>
    for AccountSubqueryResult<F>
{
    fn from_rw(rw: &zkevm_circuits::witness::Rw, block_number: u64) -> Option<Self> {
        AccountFieldIdx::from_field_tag(rw.field_tag()).map(|field_idx| {
            (
                AnySubqueryResult {
                    subquery: AccountSubquery {
                        block_number: block_number as u32,
                        addr: rw.address().unwrap(),
                        field_idx: field_idx as u32,
                    },
                    value: H256::from_uint(&rw.value_word()),
                },
                PhantomData,
            )
        })
    }

    fn assign_axiom(&self, ctx: &mut Context<F>) -> AxiomAccountPayload<F> {
        AxiomAccountPayload::<F> {
            block_number: ctx.load_witness(F::from(self.0.subquery.block_number as u64)),
            address: ctx.load_witness(self.0.subquery.addr.to_scalar().unwrap()),
            field_idx: ctx.load_witness(F::from(self.0.subquery.field_idx as u64)),
            value: HiLoRlc {
                hilo: HiLo::<F>::from(self.0.value).assign(ctx),
                rlc: ctx.load_witness(rlc::value(self.0.value.as_bytes(), F::ZERO)), // TODO take correct randomness
            },
        }
    }

    fn assign_halo2(
        &self,
        config: &InitStateTable<F>,
        layouter: &mut impl Layouter<F>,
        randomness: Value<F>,
    ) -> Halo2AccountPayload<F> {
        layouter
            .assign_region(
                || "myregion",
                |mut region| {
                    let address: F = self.0.subquery.addr.to_scalar().unwrap();
                    let field_tag: F = F::from(self.0.subquery.field_idx as u64);
                    let value_hilo = HiLo::<F>::from(self.0.value);
                    let value_rlc = randomness
                        .map(|r| Assigned::Trivial(rlc::value(self.0.value.as_bytes(), r)));
                    let offset = 0;

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

                    let assigned_field_tag = region.assign_advice(
                        || format!("is table row {offset} field_tag"),
                        config.field_tag,
                        offset,
                        || Value::known(Assigned::Trivial(field_tag)),
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

                    Ok(Halo2AccountPayload {
                        block_number: assigned_block_number,
                        address: assigned_address,
                        field_idx: assigned_field_tag,
                        value: HiLoRlc {
                            hilo: HiLo::from_hi_lo([assigned_value_hi, assigned_value_lo]),
                            rlc: assigned_value_rlc,
                        },
                    })
                },
            )
            .unwrap()
    }
}
