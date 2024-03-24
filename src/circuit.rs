use std::env::set_var;

use crate::builder::{
    SuperCircuit, ZkevmCircuitBuilder, ZkevmCircuitInput, ZkevmCircuitParams, MAX_CALLDATA,
    MAX_INNER_BLOCKS, MAX_TXS, MOCK_CHAIN_ID,
};
use axiom_eth::{
    halo2_base::gates::circuit::{BaseCircuitParams, CircuitBuilderStage},
    halo2curves::bn256::Fr,
    rlc::{circuit::RlcCircuitParams, virtual_region::RlcThreadBreakPoints},
    utils::component::{
        circuit::ComponentCircuitImpl, promise_loader::empty::EmptyPromiseLoader, ComponentCircuit,
    },
};
use bus_mapping::circuit_input_builder::{CircuitInputBuilder, CircuitsParams};
use eth_types::Word;
use zkevm_circuits::{
    super_circuit::test::block_1tx_trace,
    witness::{block_apply_mpt_state, block_convert},
};

type ZkevmCircuit = ComponentCircuitImpl<Fr, ZkevmCircuitBuilder<Fr>, EmptyPromiseLoader<Fr>>;
// use halo2_proofs::plonk::Circuit;

pub fn new() -> (u32, ZkevmCircuit, Vec<Vec<Fr>>) {
    let k = 19;

    let circuit = ZkevmCircuit::new_from_stage(
        CircuitBuilderStage::Mock,
        ZkevmCircuitParams,
        (),
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

    let MOCK_DIFFICULTY: Word = Word::from(0x200000u64);

    set_var("COINBASE", "0x0000000000000000000000000000000000000000");
    set_var("CHAIN_ID", MOCK_CHAIN_ID.to_string());
    let mut difficulty_be_bytes = [0u8; 32];
    MOCK_DIFFICULTY.to_big_endian(&mut difficulty_be_bytes);
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

    let (k, super_circuit, mut instances) =
        SuperCircuit::<Fr>::build_from_witness_block(block).unwrap();

    // let (k, super_circuit, mut instances, _) =
    //     SuperCircuit_::<Fr>::build(data, circuits_params).unwrap();

    // halo2_utils::info::print(&super_circuit);

    // let prover = MockProver::run(k, &super_circuit, instances).unwrap();
    // println!("verifying constraints");
    // prover.assert_satisfied_par();

    // println!("done");
    // return;

    circuit
        .feed_input(Box::new(ZkevmCircuitInput {
            super_circuit: Some(super_circuit.clone()),
        }))
        .unwrap();

    let public_instances = circuit.get_public_instances();
    instances.push(public_instances.into());

    return (k, circuit, instances);
    // println!("promise results fullfilled");

    // println!("instance from super_circuit {:?}", instances);

    // halo2_utils::compare::compare_all(&super_circuit, &circuit, Some(k));
    // halo2_utils::assignments::print_all(&circuit, Some(k), Some(100));
    // println!("running circuit");
    // let prover = MockProver::run(k, &circuit, instances).unwrap();
    // println!("verifying constraints");
    // prover.assert_satisfied_par();
    // println!("success!");
}
