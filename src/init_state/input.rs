use super::{
    account::{AccountPayload, AccountSubqueryResult},
    storage::{StoragePayload, StorageSubqueryResult},
};
use crate::common::Payload;
use axiom_eth::{
    halo2_base::{AssignedValue, Context},
    utils::component::promise_collector::PromiseCaller,
};
use axiom_query::{
    components::subqueries::{
        account::types::{ComponentTypeAccountSubquery, FieldAccountSubqueryCall},
        storage::types::{ComponentTypeStorageSubquery, FieldStorageSubqueryCall},
    },
    utils::codec::{AssignedAccountSubquery, AssignedStorageSubquery},
};
use eth_types::Field;
use serde::Serialize;
use std::marker::PhantomData;
use zkevm_circuits::{table::RwTableTag, witness::Block};

#[derive(Clone, Default, Debug, Serialize)]
pub struct InitState<F: Field> {
    pub accounts: Vec<AccountSubqueryResult<F>>,
    pub storages: Vec<StorageSubqueryResult<F>>,
    pub _marker: PhantomData<F>,
}

impl<F: Field> InitState<F> {
    pub fn build_from_witness_block(block: &Block<F>) -> Self
    where
        F: Field,
    {
        let block_number = block
            .txs
            .first()
            .map(|tx| tx.block_number)
            .unwrap_or_default();

        println!("account_subqueries");
        // TODO remove duplicates as in EVM same state can be accessed again
        let mut account_subqueries = vec![];
        let account_rws = block.rws.0.get(&RwTableTag::Account).unwrap();
        for rw in account_rws.iter() {
            if let Some(subquery) = AccountSubqueryResult::<F>::from_rw(rw, block_number) {
                account_subqueries.push(subquery);
            }
        }
        println!("storage_subqueries");
        let mut storage_subqueries = vec![];
        let storage_rws = block.rws.0.get(&RwTableTag::AccountStorage).unwrap();
        for rw in storage_rws.iter() {
            if let Some(subquery) = StorageSubqueryResult::<F>::from_rw(rw, block_number) {
                storage_subqueries.push(subquery);
            }
        }

        Self {
            accounts: account_subqueries,
            storages: storage_subqueries,
            _marker: PhantomData,
        }
    }

    pub fn make_account_promise_calls(
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

    pub fn make_storage_promise_calls(
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
                            slot: assigned.slot.hilo,
                        }),
                    )
                    .unwrap();
                ctx.constrain_equal(&assigned.value.hi(), &result.hi());
                ctx.constrain_equal(&assigned.value.lo(), &result.lo());

                assigned
            })
            .collect()
    }
}
