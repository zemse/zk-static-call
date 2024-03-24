use std::env::set_var;

use crate::{
    builder::{
        SuperCircuit, ZkevmCircuitBuilder, ZkevmCircuitInput, ZkevmCircuitParams, MAX_CALLDATA,
        MAX_INNER_BLOCKS, MAX_TXS, MOCK_CHAIN_ID,
    },
    init_state::InitState,
};
use axiom_eth::{
    halo2_base::gates::circuit::{BaseCircuitParams, CircuitBuilderStage},
    halo2curves::bn256::Fr,
    rlc::{circuit::RlcCircuitParams, virtual_region::RlcThreadBreakPoints},
    utils::component::{
        circuit::ComponentCircuitImpl,
        promise_loader::{
            combo::PromiseBuilderCombo,
            empty::EmptyPromiseLoader,
            single::{PromiseLoader, PromiseLoaderParams},
        },
        ComponentCircuit, ComponentType,
    },
};
use axiom_query::components::subqueries::{
    account::types::{ComponentTypeAccountSubquery, OutputAccountShard},
    common::shard_into_component_promise_results,
    storage::types::{ComponentTypeStorageSubquery, OutputStorageShard},
};
use bus_mapping::circuit_input_builder::{CircuitInputBuilder, CircuitsParams};
use eth_types::Word;
use zkevm_circuits::{
    super_circuit::test::block_1tx_trace,
    witness::{block_apply_mpt_state, block_convert},
};

type MultiPromiseLoader<F> = PromiseBuilderCombo<
    F,
    PromiseLoader<F, ComponentTypeAccountSubquery<F>>,
    PromiseLoader<F, ComponentTypeStorageSubquery<F>>,
>;

type ZkevmCircuit = ComponentCircuitImpl<Fr, ZkevmCircuitBuilder<Fr>, MultiPromiseLoader<Fr>>;

pub fn new() -> (u32, ZkevmCircuit, Vec<Vec<Fr>>) {
    let k = 19;
    let storage_capacity = 10;
    let account_capacity = 10;

    let circuit = ZkevmCircuit::new_from_stage(
        CircuitBuilderStage::Mock,
        ZkevmCircuitParams,
        (
            PromiseLoaderParams::new_for_one_shard(account_capacity),
            PromiseLoaderParams::new_for_one_shard(storage_capacity),
        ),
        RlcCircuitParams {
            base: BaseCircuitParams {
                k,
                num_advice_per_phase: vec![1, 1],
                num_fixed: 1,
                num_lookup_advice_per_phase: vec![],
                lookup_bits: Some(1),
                num_instance_columns: 1,
            },
            num_rlc_columns: 1,
        },
    )
    .use_break_points(RlcThreadBreakPoints {
        base: vec![vec![], vec![]],
        rlc: vec![],
    });

    let l2_trace = block_1tx_trace();

    let circuits_params = CircuitsParams {
        max_txs: MAX_TXS,
        max_calldata: MAX_CALLDATA,
        max_rws: 256,
        max_copy_rows: 256,
        max_exp_steps: 256,
        max_bytecode: 512,
        max_mpt_rows: 2049,
        max_poseidon_rows: 512,
        max_evm_rows: 0,
        max_keccak_rows: 0,
        max_inner_blocks: MAX_INNER_BLOCKS,
        max_rlp_rows: 500,
        ..Default::default()
    };

    let mock_difficulty: Word = Word::from(0x200000u64);

    set_var("COINBASE", "0x0000000000000000000000000000000000000000");
    set_var("CHAIN_ID", MOCK_CHAIN_ID.to_string());
    let mut difficulty_be_bytes = [0u8; 32];
    mock_difficulty.to_big_endian(&mut difficulty_be_bytes);
    // set_var("DIFFICULTY", hex::encode(difficulty_be_bytes));
    set_var("DIFFICULTY", "0");

    let mut builder =
        CircuitInputBuilder::new_from_l2_trace(circuits_params, l2_trace, false, false)
            .expect("could not handle block tx");

    builder
        .finalize_building()
        .expect("could not finalize building block");

    let mut block = block_convert(&builder.block, &builder.code_db).unwrap();
    block_apply_mpt_state(
        &mut block,
        &builder.mpt_init_state.expect("used non-light mode"),
    );

    let init_state = InitState::build_from_witness_block(&block);

    let (k, super_circuit, mut instances) =
        SuperCircuit::<Fr>::build_from_witness_block(block).unwrap();

    circuit
        .feed_input(Box::new(ZkevmCircuitInput {
            super_circuit: Some(super_circuit.clone()),
            init_state: Some(init_state.clone()),
        }))
        .unwrap();

    let promises = [
        (
            ComponentTypeAccountSubquery::<Fr>::get_type_id(),
            shard_into_component_promise_results::<Fr, ComponentTypeAccountSubquery<Fr>>(
                OutputAccountShard {
                    results: init_state
                        .clone()
                        .accounts
                        .iter()
                        .map(|a| a.0.clone())
                        .collect(),
                }
                .into(),
            ),
        ),
        (
            ComponentTypeStorageSubquery::<Fr>::get_type_id(),
            shard_into_component_promise_results::<Fr, ComponentTypeStorageSubquery<Fr>>(
                OutputStorageShard {
                    results: init_state
                        .clone()
                        .storages
                        .iter()
                        .map(|a| a.0.clone())
                        .collect(),
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
    instances.push(public_instances.into());

    (k, circuit, instances)
}
