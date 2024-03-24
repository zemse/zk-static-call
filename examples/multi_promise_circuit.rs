#![feature(slice_flatten)]
use account::AccountPayload;
use axiom_codec::{
    types::{
        field_elements::AnySubqueryResult,
        native::{AccountSubquery, StorageSubquery},
    },
    HiLo,
};
use axiom_eth::{
    halo2_base::{AssignedValue, Context},
    halo2_proofs::{circuit::*, dev::MockProver, plonk::*},
    utils::{
        build_utils::dummy::DummyFrom,
        component::{
            circuit::*,
            promise_collector::PromiseCaller,
            promise_loader::combo::PromiseBuilderCombo,
            types::{EmptyComponentType, LogicalEmpty},
        },
    },
    zkevm_hashes::util::eth_types::ToScalar,
    // Field,
};
use axiom_query::{
    components::subqueries::{
        account::types::{
            ComponentTypeAccountSubquery, FieldAccountSubqueryCall, OutputAccountShard,
        },
        storage::types::{
            ComponentTypeStorageSubquery, FieldStorageSubqueryCall, OutputStorageShard,
        },
    },
    utils::codec::{AssignedAccountSubquery, AssignedStorageSubquery},
};
use eth_types::Field;
use ethers_core::types::{H256, U256};
use serde::Serialize;
use std::marker::PhantomData;
use storage::StoragePayload;
use zkevm_circuits::evm_circuit::util::rlc;

type AxiomAssignedValue<F> = AssignedValue<F>;
type Halo2AssignedCell<F> = AssignedCell<Assigned<F>, F>;

trait Assign<F: Field, A, H> {
    fn assign_axiom(&self, ctx: &mut Context<F>) -> A;

    fn assign_halo2(
        &self,
        config: &init_state::InitStateTable,
        layouter: &mut impl Layouter<F>,
        randomness: Value<F>,
    ) -> H;
}

mod init_state {
    use eth_types::{BigEndianHash, ToLittleEndian};
    use zkevm_circuits::{
        evm_circuit::util::rlc,
        table::{LookupTable, RwTableTag},
        witness::Block,
    };

    use super::*;

    /// Tag to identify the field in a Init State Table row
    /// Keep the sequence consistent with OpcodeId for scalar
    #[derive(Clone, Copy, Debug)]
    pub enum FieldTag {
        /// Storage field, field_tag is zero for AccountStorage
        Storage = 0,
        /// Nonce field
        Nonce = 1,
        /// Balance field
        Balance = 2,
        /// CodeHash field
        CodeHash = 3,
    }

    #[derive(Clone, Default, Debug, Serialize)]
    pub struct InitState<F: Field> {
        pub accounts: Vec<account::AccountSubqueryResult>,
        pub storages: Vec<storage::StorageSubqueryResult>,
        pub _marker: PhantomData<F>,
    }

    impl<F: Field> InitState<F> {
        pub fn from_witness_block(block: &Block<F>) -> Self {
            let block_number = block
                .txs
                .first()
                .map(|tx| tx.block_number)
                .unwrap_or_default();

            // TODO remove duplicates as in EVM same state can be accessed again
            let mut account_subqueries = vec![];
            let account_rws = block.rws.0.get(&RwTableTag::Account).unwrap();
            for rw in account_rws.iter() {
                account_subqueries.push(account::AccountSubqueryResult {
                    subquery: AccountSubquery {
                        block_number: block_number as u32,
                        addr: rw.address().unwrap(),
                        field_idx: match rw.field_tag().unwrap() {
                            1 => FieldTag::Nonce as u32,
                            2 => FieldTag::Balance as u32,
                            3 => FieldTag::CodeHash as u32,
                            _ => unreachable!(),
                        },
                    },
                    value: H256::from_uint(&rw.account_value_pair().0),
                });
            }

            let mut storage_subqueries = vec![];
            let storage_rws = block.rws.0.get(&RwTableTag::AccountStorage).unwrap();
            for rw in storage_rws.iter() {
                storage_subqueries.push(storage::StorageSubqueryResult {
                    subquery: StorageSubquery {
                        block_number: block_number as u32,
                        addr: rw.address().unwrap(),
                        slot: rw.storage_key().unwrap(),
                    },
                    value: H256::from_uint(&rw.account_value_pair().0),
                });
            }

            Self {
                accounts: account_subqueries,
                storages: storage_subqueries,
                _marker: PhantomData,
            }
        }

        pub fn accounts_assigned(
            &self,
            ctx: &mut Context<F>,
            promise_caller: &PromiseCaller<F>,
        ) -> Vec<AccountPayload<AssignedValue<F>>> {
            self.accounts
                .iter()
                .map(|q| {
                    let assigned = q.assign_axiom(ctx);
                    let result = promise_caller
                        .call::<FieldAccountSubqueryCall<F>, ComponentTypeAccountSubquery<F>>(
                            ctx,
                            FieldAccountSubqueryCall(AssignedAccountSubquery {
                                block_number: assigned.block_number,
                                addr: assigned.address,
                                field_idx: assigned.field_idx,
                            }),
                        )
                        .unwrap();
                    ctx.constrain_equal(&assigned.value_hilo.hi(), &result.hi());
                    ctx.constrain_equal(&assigned.value_hilo.lo(), &result.lo());

                    assigned
                })
                .collect()
        }
        pub fn storage_assigned(
            &self,
            ctx: &mut Context<F>,
            promise_caller: &PromiseCaller<F>,
        ) -> Vec<StoragePayload<AssignedValue<F>>> {
            self.storages
                .iter()
                .map(|p| {
                    let assigned = p.assign_axiom(ctx);
                    let result = promise_caller
                        .call::<FieldStorageSubqueryCall<F>, ComponentTypeStorageSubquery<F>>(
                            ctx,
                            FieldStorageSubqueryCall(AssignedStorageSubquery {
                                block_number: assigned.block_number,
                                addr: assigned.address,
                                slot: assigned.slot,
                            }),
                        )
                        .unwrap();

                    ctx.constrain_equal(&assigned.value.hi(), &result.hi());
                    ctx.constrain_equal(&assigned.value.lo(), &result.lo());

                    assigned
                })
                .collect()
        }

        fn assign_halo2(
            &self,
            config: &init_state::InitStateTable,
            layouter: &mut impl Layouter<F>,
        ) -> Vec<Halo2AssignedCell<F>> {
            todo!()
        }
    }

    #[derive(Clone)]
    pub struct InitStateTable {
        pub block_number: Column<Advice>, // TODO remove this
        pub address: Column<Advice>,
        pub field_tag: Column<Advice>,
        pub storage_key_hi: Column<Advice>,
        pub storage_key_lo: Column<Advice>,
        pub storage_key_rlc: Column<Advice>,
        pub value_hi: Column<Advice>,
        pub value_lo: Column<Advice>,
        pub value_rlc: Column<Advice>,
    }

    impl<F: Field> LookupTable<F> for InitStateTable {
        fn columns(&self) -> Vec<Column<Any>> {
            vec![
                self.address.into(),
                self.field_tag.into(),
                self.storage_key_hi.into(),
                self.storage_key_lo.into(),
                self.storage_key_rlc.into(),
                self.value_hi.into(),
                self.value_lo.into(),
                self.value_rlc.into(),
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

    impl InitStateTable {
        /// Construct a new InitStateTable
        pub fn construct<F: Field>(meta: &mut ConstraintSystem<F>) -> Self {
            Self {
                block_number: meta.advice_column(),
                address: meta.advice_column(),
                field_tag: meta.advice_column(),
                storage_key_hi: meta.advice_column(),
                storage_key_lo: meta.advice_column(),
                storage_key_rlc: meta.advice_column(),
                value_hi: meta.advice_column(),
                value_lo: meta.advice_column(),
                value_rlc: meta.advice_column(),
            }
        }

        /// Assign the `InitStateTable` from a `RwMap`.
        pub fn assign_raw_halo2<F: Field>(
            &self,
            layouter: &mut impl Layouter<F>,
            block: &Block<F>,
            randomness: Value<F>,
        ) -> Result<(), Error> {
            // let init_state = InitState::from_witness_block(block);

            // TODO use init_state instead of accessing rws again, as it will have filtered duplicates
            layouter.assign_region(
                || "is table",
                |mut region| {
                    let is_table_columns = <InitStateTable as LookupTable<F>>::advice_columns(self);
                    let mut offset = 0;

                    for column in is_table_columns {
                        region.assign_advice(
                            || "is table all-zero row",
                            column,
                            offset,
                            || Value::known(F::ZERO),
                        )?;
                    }
                    offset += 1;

                    // TODO remove duplicates
                    let account_rws = block.rws.0.get(&RwTableTag::Account).unwrap();
                    for rw in account_rws.iter() {
                        let address: F = rw.address().unwrap().to_scalar().unwrap();
                        let field_tag: F = F::from(match rw.field_tag().unwrap() {
                            1 => FieldTag::Nonce as u64,
                            2 => FieldTag::Balance as u64,
                            3 => FieldTag::CodeHash as u64,
                            _ => unreachable!(),
                        });

                        let value_rlc = randomness.map(|r| rw.value_assignment(r));
                        let value_hilo: HiLo<F> = HiLo::from(H256::from_uint(&rw.value_word()));

                        let assigned_address = region.assign_advice(
                            || format!("is table row {offset} address"),
                            self.address,
                            offset,
                            || Value::known(address),
                        )?;
                        let assigned_field_tag = region.assign_advice(
                            || format!("is table row {offset} field_tag"),
                            self.field_tag,
                            offset,
                            || Value::known(field_tag),
                        )?;
                        let assigned_storage_key_hi = region.assign_advice(
                            || format!("is table row {offset} storage_key hi"),
                            self.storage_key_hi,
                            offset,
                            || Value::known(F::ZERO),
                        )?;
                        let assigned_storage_key_lo = region.assign_advice(
                            || format!("is table row {offset} storage_key lo"),
                            self.storage_key_lo,
                            offset,
                            || Value::known(F::ZERO),
                        )?;
                        let assigned_storage_key_rlc = region.assign_advice(
                            || format!("is table row {offset} storage_key rlc"),
                            self.storage_key_rlc,
                            offset,
                            || Value::known(F::ZERO),
                        )?;
                        let assigned_value_hi = region.assign_advice(
                            || format!("is table row {offset} value hi"),
                            self.value_hi,
                            offset,
                            || Value::known(value_hilo.hi()),
                        )?;
                        let assigned_value_lo = region.assign_advice(
                            || format!("is table row {offset} value lo"),
                            self.value_lo,
                            offset,
                            || Value::known(value_hilo.lo()),
                        )?;
                        let assigned_value_rlc = region.assign_advice(
                            || format!("is table row {offset} value rlc"),
                            self.value_rlc,
                            offset,
                            || value_rlc,
                        )?;
                        offset += 1;
                    }

                    // TODO remove duplicates
                    let storage_rws = block.rws.0.get(&RwTableTag::AccountStorage).unwrap();
                    for rw in storage_rws.iter() {
                        let address: F = rw.address().unwrap().to_scalar().unwrap();
                        let field_tag: F = F::from(FieldTag::Storage as u64);
                        let key_hilo: HiLo<F> =
                            HiLo::from(H256::from_uint(&rw.storage_key().unwrap()));
                        let key_rlc = randomness
                            .map(|r| rlc::value(&rw.storage_key().unwrap().to_le_bytes(), r));
                        let value_hilo: HiLo<F> = HiLo::from(H256::from_uint(&rw.value_word()));
                        let value_rlc = randomness.map(|r| rw.value_assignment(r));

                        let assigned_address = region.assign_advice(
                            || format!("is table row {offset} address"),
                            self.address,
                            offset,
                            || Value::known(address),
                        )?;
                        let assigned_field_tag = region.assign_advice(
                            || format!("is table row {offset} field_tag"),
                            self.field_tag,
                            offset,
                            || Value::known(field_tag),
                        )?;
                        let assigned_storage_key_hi = region.assign_advice(
                            || format!("is table row {offset} storage_key hi"),
                            self.storage_key_hi,
                            offset,
                            || Value::known(key_hilo.hi()),
                        )?;
                        let assigned_storage_key_lo = region.assign_advice(
                            || format!("is table row {offset} storage_key lo"),
                            self.storage_key_lo,
                            offset,
                            || Value::known(key_hilo.lo()),
                        )?;
                        let assigned_storage_key_rlc = region.assign_advice(
                            || format!("is table row {offset} storage_key rlc"),
                            self.storage_key_rlc,
                            offset,
                            || key_rlc,
                        )?;
                        let assigned_value_rlc = region.assign_advice(
                            || format!("is table row {offset} value rlc"),
                            self.value_rlc,
                            offset,
                            || value_rlc,
                        )?;
                        let assigned_value_hi = region.assign_advice(
                            || format!("is table row {offset} value hi"),
                            self.value_hi,
                            offset,
                            || Value::known(value_hilo.hi()),
                        )?;
                        let assigned_value_lo = region.assign_advice(
                            || format!("is table row {offset} value lo"),
                            self.value_lo,
                            offset,
                            || Value::known(value_hilo.lo()),
                        )?;
                        offset += 1;
                    }

                    Ok(())
                },
            )
        }
    }
}

mod account {
    use super::*;

    #[derive(Clone, Copy)]
    pub enum FieldIdx {
        Nonce = 0,
        Balance = 1,
        StorageRoot = 2,
        CodeHash = 3,
    }

    pub struct AccountPayload<AssignedType> {
        pub block_number: AssignedType, // TODO constrain the block number later
        pub address: AssignedType,
        pub field_idx: AssignedType,
        pub value_hilo: HiLo<AssignedType>,
    }

    pub type AccountSubqueryResult = AnySubqueryResult<AccountSubquery, H256>;
    pub type AxiomAccountPayload<F> = AccountPayload<AxiomAssignedValue<F>>;
    pub type Halo2AccountPayload<F> = AccountPayload<Halo2AssignedCell<F>>;

    impl<F: Field> Assign<F, AxiomAccountPayload<F>, Halo2AccountPayload<F>> for AccountSubqueryResult {
        fn assign_axiom(&self, ctx: &mut Context<F>) -> AxiomAccountPayload<F> {
            AxiomAccountPayload::<F> {
                block_number: ctx.load_witness(F::from(self.subquery.block_number as u64)),
                address: ctx.load_witness(self.subquery.addr.to_scalar().unwrap()),
                field_idx: ctx.load_witness(F::from(self.subquery.field_idx as u64)),
                value_hilo: HiLo::<F>::from(self.value).assign(ctx),
            }
        }

        fn assign_halo2(
            &self,
            config: &init_state::InitStateTable,
            layouter: &mut impl Layouter<F>,
            randomness: Value<F>,
        ) -> Halo2AccountPayload<F> {
            layouter
                .assign_region(
                    || "myregion",
                    |mut region| {
                        let mut offset = 0;
                        // let account_rws = self.block.rws.0.get(&RwTableTag::Account).unwrap();

                        let address: F = self.subquery.addr.to_scalar().unwrap();
                        let field_tag: F = F::from(match self.subquery.field_idx {
                            1 => account::FieldIdx::Nonce as u64,
                            2 => account::FieldIdx::Balance as u64,
                            3 => account::FieldIdx::CodeHash as u64,
                            _ => unreachable!(),
                        });

                        let value_rlc = randomness
                            .map(|r| Assigned::Trivial(rlc::value(self.value.as_bytes(), r)));
                        let value_hilo = HiLo::<F>::from(self.value);
                        // let value_hilo = HiLo::from_hi_lo(
                        //     HiLo::<F>::from(self.value).hi_lo().map(Assigned::Trivial),
                        // );

                        let assigned_block_number = region.assign_advice(
                            || format!("is table row {offset} block_number"),
                            config.block_number,
                            offset,
                            || {
                                Value::known(Assigned::Trivial(F::from(
                                    self.subquery.block_number as u64,
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
                        let assigned_storage_key_hi = region.assign_advice(
                            || format!("is table row {offset} storage_key hi"),
                            config.storage_key_hi,
                            offset,
                            || Value::known(Assigned::Trivial(F::ZERO)),
                        )?;
                        let assigned_storage_key_lo = region.assign_advice(
                            || format!("is table row {offset} storage_key lo"),
                            config.storage_key_lo,
                            offset,
                            || Value::known(Assigned::Trivial(F::ZERO)),
                        )?;
                        // TODO add gate which ensures rlc == hilo
                        let assigned_storage_key_rlc = region.assign_advice(
                            || format!("is table row {offset} storage_key rlc"),
                            config.storage_key_rlc,
                            offset,
                            || Value::known(Assigned::Trivial(F::ZERO)),
                        )?;
                        let assigned_value_hi = region.assign_advice(
                            || format!("is table row {offset} value hi"),
                            config.value_hi,
                            offset,
                            || Value::known(Assigned::Trivial(value_hilo.hi())),
                        )?;
                        let assigned_value_lo = region.assign_advice(
                            || format!("is table row {offset} value lo"),
                            config.value_lo,
                            offset,
                            || Value::known(Assigned::Trivial(value_hilo.lo())),
                        )?;
                        // TODO add gate which ensures rlc == hilo
                        let assigned_value_rlc = region.assign_advice(
                            || format!("is table row {offset} value rlc"),
                            config.value_rlc,
                            offset,
                            || value_rlc,
                        )?;

                        offset += 1;

                        Ok(Halo2AccountPayload {
                            block_number: assigned_block_number,
                            address: assigned_address,
                            field_idx: assigned_field_tag,
                            value_hilo: HiLo::from_hi_lo([assigned_value_hi, assigned_value_lo]),
                        })
                    },
                )
                .unwrap()
        }
    }
}

mod storage {
    use eth_types::BigEndianHash;

    use super::*;

    pub struct StoragePayload<AssignedType> {
        pub block_number: AssignedType,
        pub address: AssignedType,
        pub slot: HiLo<AssignedType>,
        pub value: HiLo<AssignedType>,
    }

    pub type StorageSubqueryResult = AnySubqueryResult<StorageSubquery, H256>;
    pub type AxiomStoragePayload<F> = StoragePayload<AxiomAssignedValue<F>>;
    pub type Halo2StoragePayload<F> = StoragePayload<Halo2AssignedCell<F>>;

    impl<F: Field> Assign<F, AxiomStoragePayload<F>, Halo2StoragePayload<F>> for StorageSubqueryResult {
        fn assign_axiom(&self, ctx: &mut Context<F>) -> AxiomStoragePayload<F> {
            AxiomStoragePayload {
                block_number: ctx.load_witness(F::from(self.subquery.block_number as u64)),
                address: ctx.load_witness(self.subquery.addr.to_scalar().unwrap()),
                slot: HiLo::<F>::from(H256::from_uint(&self.subquery.slot)).assign(ctx),
                value: HiLo::<F>::from(self.value).assign(ctx),
            }
        }

        fn assign_halo2(
            &self,
            config: &init_state::InitStateTable,
            layouter: &mut impl Layouter<F>,
            randomness: Value<F>,
        ) -> Halo2StoragePayload<F> {
            layouter
                .assign_region(
                    || "myregion",
                    |mut region| {
                        let mut offset = 0;

                        let address: F = self.subquery.addr.to_scalar().unwrap();
                        let field_tag: F = F::ZERO;

                        let key = H256::from_uint(&self.subquery.slot);
                        let key_rlc = randomness.map(|r| rlc::value(key.as_bytes(), r));
                        let key_hilo = HiLo::<F>::from(key);

                        let value_rlc = randomness.map(|r| rlc::value(self.value.as_bytes(), r));
                        let value_hilo = HiLo::<F>::from(self.value);

                        let assigned_block_number = region.assign_advice(
                            || format!("is table row {offset} block_number"),
                            config.block_number,
                            offset,
                            || {
                                Value::known(Assigned::Trivial(F::from(
                                    self.subquery.block_number as u64,
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
                        let assigned_storage_key_hi = region.assign_advice(
                            || format!("is table row {offset} storage_key hi"),
                            config.storage_key_hi,
                            offset,
                            || Value::known(Assigned::Trivial(key_hilo.hi())),
                        )?;
                        let assigned_storage_key_lo = region.assign_advice(
                            || format!("is table row {offset} storage_key lo"),
                            config.storage_key_lo,
                            offset,
                            || Value::known(Assigned::Trivial(key_hilo.lo())),
                        )?;
                        // TODO add gate which ensures rlc == hilo
                        let assigned_storage_key_rlc = region.assign_advice(
                            || format!("is table row {offset} storage_key rlc"),
                            config.storage_key_rlc,
                            offset,
                            || key_rlc.map(Assigned::Trivial),
                        )?;
                        // TODO add gate which ensures rlc == hilo
                        let assigned_value_rlc = region.assign_advice(
                            || format!("is table row {offset} value rlc"),
                            config.value_rlc,
                            offset,
                            || value_rlc,
                        )?;
                        let assigned_value_hi = region.assign_advice(
                            || format!("is table row {offset} value hi"),
                            config.value_hi,
                            offset,
                            || Value::known(Assigned::Trivial(value_hilo.hi())),
                        )?;
                        let assigned_value_lo = region.assign_advice(
                            || format!("is table row {offset} value lo"),
                            config.value_lo,
                            offset,
                            || Value::known(Assigned::Trivial(value_hilo.lo())),
                        )?;
                        offset += 1;

                        Ok(Halo2StoragePayload {
                            block_number: assigned_block_number,
                            address: assigned_address,
                            slot: HiLo::from_hi_lo([
                                assigned_storage_key_hi,
                                assigned_storage_key_lo,
                            ]),
                            value: HiLo::from_hi_lo([assigned_value_hi, assigned_value_lo]),
                        })
                    },
                )
                .unwrap()
        }
    }
}

// #[derive(Clone, Default, Debug, Serialize)]
// pub struct MultiInputs<F: Field> {
//     init_state: init_state::InitState,
//     _marker: PhantomData<F>,
// }

// #[derive(Clone, Default, Debug, Serialize, Deserialize)]
// pub struct AccountInput<F: Field> {
//     pub block_number: u64,
//     pub address: Address,
//     pub field_idx: u64,
//     pub value: H256,
//     pub _marker: PhantomData<F>,
// }

// pub struct AxiomAccountPayload<F: Field> {
//     pub block_number: AxiomAssignedValue<F>,
//     pub address: AxiomAssignedValue<F>,
//     pub field_idx: AxiomAssignedValue<F>,
//     pub value: HiLo<AxiomAssignedValue<F>>,
// }

// impl<F: Field> AccountInput<F> {
//     pub fn assign_axiom(&self, ctx: &mut Context<F>) -> AxiomAccountPayload<F> {
//         AxiomAccountPayload {
//             block_number: ctx.load_witness(F::from(self.block_number)),
//             address: ctx.load_witness(self.address.to_scalar().unwrap()),
//             field_idx: ctx.load_witness(F::from(self.field_idx)),
//             value: HiLo::<F>::from(self.value).assign(ctx),
//         }
//     }
// }

// #[derive(Clone, Default, Debug, Serialize, Deserialize)]
// pub struct StorageInput<F: Field> {
//     pub block_number: u64,
//     pub address: Address,
//     pub slot: H256,
//     pub value: H256,
//     pub _marker: PhantomData<F>,
// }

// #[derive(Clone)]
// pub struct MultiConfig {
//     advice: Column<Advice>,
// }

#[derive(Clone, Default)]
pub struct MultiInputParams;

impl CoreBuilderParams for MultiInputParams {
    fn get_output_params(&self) -> CoreBuilderOutputParams {
        CoreBuilderOutputParams::new(vec![])
    }
}
impl<F: Field> DummyFrom<MultiInputParams> for init_state::InitState<F> {
    fn dummy_from(_seed: MultiInputParams) -> Self {
        init_state::InitState::default()
    }
}

pub struct MultiInputsCircuitBuilder<F: Field> {
    input: Option<init_state::InitState<F>>,
}

impl<F: Field> ComponentBuilder<F> for MultiInputsCircuitBuilder<F> {
    type Config = init_state::InitStateTable;

    type Params = MultiInputParams;

    fn new(_params: Self::Params) -> Self {
        Self { input: None }
    }

    fn get_params(&self) -> Self::Params {
        MultiInputParams
    }

    fn configure_with_params(
        meta: &mut axiom_eth::halo2_proofs::plonk::ConstraintSystem<F>,
        _params: Self::Params,
    ) -> Self::Config {
        Self::Config {
            address: meta.advice_column(),
            field_tag: meta.advice_column(),
            block_number: meta.advice_column(),
            storage_key_hi: meta.advice_column(),
            storage_key_lo: meta.advice_column(),
            storage_key_rlc: meta.advice_column(),
            value_hi: meta.advice_column(),
            value_lo: meta.advice_column(),
            value_rlc: meta.advice_column(),
        }
    }

    fn calculate_params(&mut self) -> Self::Params {
        MultiInputParams
    }
}

impl<F: Field> CoreBuilder<F> for MultiInputsCircuitBuilder<F> {
    type CompType = EmptyComponentType<F>;

    type PublicInstanceValue = LogicalEmpty<F>;

    type PublicInstanceWitness = LogicalEmpty<AssignedValue<F>>;

    type CoreInput = init_state::InitState<F>;

    fn feed_input(&mut self, input: Self::CoreInput) -> anyhow::Result<()> {
        self.input = Some(input);
        Ok(())
    }

    fn virtual_assign_phase0(
        &mut self,
        // TODO: This could be replaced with a more generic CircuitBuilder. Question: can be CircuitBuilder treated as something like PromiseCircuit?
        builder: &mut axiom_eth::rlc::circuit::builder::RlcCircuitBuilder<F>,
        // Core circuits can make promise calls.
        promise_caller: axiom_eth::utils::component::promise_collector::PromiseCaller<F>,
        // TODO: Output commitmment
    ) -> CoreBuilderOutput<F, Self::CompType> {
        println!("virtual_assign_phase0 my");

        let ctx = builder.base.main(0);

        let input = self.input.as_ref().unwrap();
        let accounts = input.accounts_assigned(ctx, &promise_caller);
        let storages = input.storage_assigned(ctx, &promise_caller);

        CoreBuilderOutput {
            public_instances: vec![],
            virtual_table: vec![],
            logical_results: vec![],
        }
    }

    fn raw_synthesize_phase0(
        &mut self,
        _config: &Self::Config,
        _layouter: &mut impl axiom_eth::halo2_proofs::circuit::Layouter<F>,
    ) {
        println!("raw_synthesize_phase0 my");
    }

    fn virtual_assign_phase1(
        &mut self,
        _builder: &mut axiom_eth::rlc::circuit::builder::RlcCircuitBuilder<F>,
    ) {
        println!("virtual_assign_phase1 my");
    }

    fn raw_synthesize_phase1(
        &mut self,
        _config: &Self::Config,
        _layouter: &mut impl axiom_eth::halo2_proofs::circuit::Layouter<F>,
    ) {
        println!("raw_synthesize_phase1 my");
    }
}

#[tokio::main]
async fn main() {
    use axiom_eth::{
        halo2curves::bn256::Fr,
        providers::setup_provider,
        utils::component::{
            circuit::ComponentCircuitImpl,
            promise_loader::single::{PromiseLoader, PromiseLoaderParams},
            ComponentCircuit, ComponentType,
        },
    };
    use axiom_query::components::{
        dummy_rlc_circuit_params, subqueries::common::shard_into_component_promise_results,
    };
    use ethers_core::types::{BigEndianHash, Chain, H256};
    use ethers_providers::Middleware;
    use std::marker::PhantomData;

    use axiom_query::components::subqueries::{
        account::types::ComponentTypeAccountSubquery, storage::types::ComponentTypeStorageSubquery,
    };

    type MultiPromiseLoader<F> = PromiseBuilderCombo<
        F,
        PromiseLoader<F, ComponentTypeAccountSubquery<F>>,
        PromiseLoader<F, ComponentTypeStorageSubquery<F>>,
    >;

    pub type MultiInputCircuit =
        ComponentCircuitImpl<Fr, MultiInputsCircuitBuilder<Fr>, MultiPromiseLoader<Fr>>;

    let k = 19;
    let storage_capacity = 10;
    let account_capacity = 10;

    let block_number = 19211974; // random block from 12 feb 2024

    // input from the witness
    let account_inputs: Vec<(&str, account::FieldIdx)> = vec![
        (
            "0x60594a405d53811d3bc4766596efd80fd545a270",
            account::FieldIdx::Nonce,
        ),
        (
            "0x60594a405d53811d3bc4766596efd80fd545a270",
            account::FieldIdx::Balance,
        ),
        (
            "0x60594a405d53811d3bc4766596efd80fd545a270",
            account::FieldIdx::StorageRoot,
        ),
    ];
    let storage_inputs = vec![
        ("0x60594a405d53811d3bc4766596efd80fd545a270", H256::zero()),
        (
            "0x60594a405d53811d3bc4766596efd80fd545a270",
            H256::from_uint(&U256::one()),
        ),
        (
            "0x60594a405d53811d3bc4766596efd80fd545a270",
            H256::from_uint(&U256::from(2)),
        ),
        (
            "0x60594a405d53811d3bc4766596efd80fd545a270",
            H256::from_uint(&U256::from(3)),
        ),
    ];

    // query data from rpc
    let provider = setup_provider(Chain::Mainnet);

    let mut account_subqueries = vec![];
    for (address, field_idx) in account_inputs {
        let proof = provider
            .get_proof(address, vec![], Some(block_number.into()))
            .await
            .unwrap();
        assert_eq!(proof.storage_proof.len(), 0);
        account_subqueries.push(account::AccountSubqueryResult {
            subquery: AccountSubquery {
                block_number: block_number as u32,
                addr: proof.address,
                field_idx: field_idx as u32,
            },
            value: match field_idx {
                account::FieldIdx::Nonce => H256::from_uint(&U256::from(proof.nonce.as_u64())),
                account::FieldIdx::Balance => H256::from_uint(&proof.balance),
                account::FieldIdx::StorageRoot => proof.storage_hash,
                account::FieldIdx::CodeHash => proof.code_hash,
            },
        });
    }

    let mut storage_subqueries = vec![];
    for (address, slot) in storage_inputs {
        let proof = provider
            .get_proof(address, vec![slot], Some(block_number.into()))
            .await
            .unwrap();
        assert_eq!(proof.storage_proof.len(), 1);
        // let proof = json_to_mpt_input(proof, 13, 0);
        storage_subqueries.push(storage::StorageSubqueryResult {
            subquery: StorageSubquery {
                block_number: block_number as u32,
                addr: proof.address,
                slot: proof.storage_proof[0].key.into_uint(),
            },
            value: H256::from_uint(&proof.storage_proof[0].value),
        });
    }

    let circuit_input = init_state::InitState::<Fr> {
        accounts: account_subqueries,
        storages: storage_subqueries,
        _marker: PhantomData,
    };

    let mut circuit = MultiInputCircuit::new(
        MultiInputParams,
        (
            PromiseLoaderParams::new_for_one_shard(account_capacity),
            PromiseLoaderParams::new_for_one_shard(storage_capacity),
        ),
        dummy_rlc_circuit_params(k as usize),
    );
    circuit.feed_input(Box::new(circuit_input.clone())).unwrap();
    circuit.calculate_params();
    let promises = [
        (
            ComponentTypeAccountSubquery::<Fr>::get_type_id(),
            shard_into_component_promise_results::<Fr, ComponentTypeAccountSubquery<Fr>>(
                OutputAccountShard {
                    results: circuit_input.accounts.clone(),
                }
                .into(),
            ),
        ),
        (
            ComponentTypeStorageSubquery::<Fr>::get_type_id(),
            shard_into_component_promise_results::<Fr, ComponentTypeStorageSubquery<Fr>>(
                OutputStorageShard {
                    results: circuit_input.storages,
                }
                .into(),
            ),
        ),
    ]
    .into_iter()
    .collect();
    circuit.fulfill_promise_results(&promises).unwrap();
    println!("promise results fullfilled");
    let public_instances = circuit.get_public_instances();
    let instances = vec![public_instances.into()];

    println!("{:?}", instances);

    // halo2_utils::info::print(&circuit);

    println!("running circuit");
    let prover = MockProver::run(k, &circuit, instances).unwrap();
    println!("verifying constraints");
    prover.assert_satisfied();
}
