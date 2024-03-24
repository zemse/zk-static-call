use std::env::set_var;

/// Zkevm Circuit
///
/// Prove a block using Axiom Component Framework
///
use axiom_eth::{
    halo2_base::AssignedValue,
    halo2_proofs::{
        circuit::Layouter,
        dev::MockProver,
        halo2curves::bn256::Fr,
        plonk::{Circuit, ConstraintSystem},
    },
    rlc::circuit::builder::RlcCircuitBuilder,
    utils::{
        build_utils::dummy::DummyFrom,
        component::{
            circuit::{
                ComponentBuilder, CoreBuilder, CoreBuilderOutput, CoreBuilderOutputParams,
                CoreBuilderParams,
            },
            promise_collector::PromiseCaller,
            promise_loader::empty::EmptyPromiseLoader,
            types::{EmptyComponentType, LogicalEmpty},
        },
    },
    // Field,
};

use bus_mapping::circuit_input_builder::{CircuitInputBuilder, CircuitsParams, PrecompileEcParams};
// use bus_mapping::circuit_input_builder::{FeatureConfig, FixedCParams};
use eth_types::{Field, Word};
use ethers_core::utils::hex;
use zkevm_circuits::{
    super_circuit::{test::block_1tx_trace, SuperCircuit, SuperCircuitConfig},
    util::Challenges,
    witness::{block_apply_mpt_state, block_convert},
};

const MAX_TXS: usize = 1;
const MAX_CALLDATA: usize = 256;
const MAX_INNER_BLOCKS: usize = 1;
const TEST_MOCK_RANDOMNESS: u64 = 0x100;

const MOCK_CHAIN_ID: u64 = 1338;
// const MOCK_DIFFICULTY: Word = Word::from(0x200000u64);

type SuperCircuit_<F> =
    SuperCircuit<F, MAX_TXS, MAX_CALLDATA, MAX_INNER_BLOCKS, TEST_MOCK_RANDOMNESS>;

#[derive(Clone, Default)]
pub struct ZkevmCircuitParams;

impl CoreBuilderParams for ZkevmCircuitParams {
    fn get_output_params(&self) -> CoreBuilderOutputParams {
        // TODO see what this means
        CoreBuilderOutputParams::new(vec![1])
    }
}

// Private inputs to our circuit
#[derive(Clone, Default, Debug)]
pub struct ZkevmCircuitInput<F: Field> {
    super_circuit: Option<SuperCircuit_<F>>,
}

impl<F: Field> DummyFrom<ZkevmCircuitParams> for ZkevmCircuitInput<F> {
    fn dummy_from(_seed: ZkevmCircuitParams) -> Self {
        ZkevmCircuitInput {
            super_circuit: None,
        }
    }
}

// Raw halo2 configuration
#[derive(Clone)]
pub struct ZkevmCircuitConfig {
    super_circuit: (SuperCircuitConfig<Fr>, Challenges),
}

// TODO reason why we have a circuit component struct as well as ZkevmCircuitInput
pub struct ZkevmCircuitBuilder<F: Field> {
    input: ZkevmCircuitInput<F>,
}

impl ComponentBuilder<Fr> for ZkevmCircuitBuilder<Fr> {
    type Config = ZkevmCircuitConfig;

    type Params = ZkevmCircuitParams;

    fn new(_params: Self::Params) -> Self {
        Self {
            input: ZkevmCircuitInput {
                super_circuit: None,
            },
        }
    }

    fn get_params(&self) -> Self::Params {
        ZkevmCircuitParams
    }

    fn configure_with_params(
        _meta: &mut ConstraintSystem<Fr>,
        _params: Self::Params,
    ) -> Self::Config {
        ZkevmCircuitConfig {
            super_circuit: SuperCircuit_::<Fr>::configure(_meta),
        }
    }

    fn calculate_params(&mut self) -> Self::Params {
        ZkevmCircuitParams
    }
}

impl CoreBuilder<Fr> for ZkevmCircuitBuilder<Fr> {
    type CompType = EmptyComponentType<Fr>;

    type PublicInstanceValue = LogicalEmpty<Fr>;

    type PublicInstanceWitness = LogicalEmpty<AssignedValue<Fr>>;

    type CoreInput = ZkevmCircuitInput<Fr>;

    fn feed_input(&mut self, input: Self::CoreInput) -> anyhow::Result<()> {
        // println!("feed_input {:?}", input);
        self.input = input;
        Ok(())
    }

    fn virtual_assign_phase0(
        &mut self,
        _builder: &mut RlcCircuitBuilder<Fr>,
        _promise_caller: PromiseCaller<Fr>,
    ) -> CoreBuilderOutput<Fr, Self::CompType> {
        println!("virtual_assign_phase0");

        CoreBuilderOutput {
            public_instances: vec![],
            virtual_table: vec![],
            logical_results: vec![],
        }
    }

    fn raw_synthesize_phase0(&mut self, config: &Self::Config, layouter: &mut impl Layouter<Fr>) {
        println!("raw_synthesize_phase0");
        self.input
            .super_circuit
            .as_ref()
            .unwrap()
            .synthesize_2(config.super_circuit.clone(), layouter)
            .unwrap();
    }

    fn virtual_assign_phase1(&mut self, _builder: &mut RlcCircuitBuilder<Fr>) {
        println!("virtual_assign_phase1");
    }

    fn raw_synthesize_phase1(&mut self, _config: &Self::Config, _layouter: &mut impl Layouter<Fr>) {
        println!("raw_synthesize_phase1");
    }
}

use axiom_eth::{
    halo2_base::gates::circuit::{BaseCircuitParams, CircuitBuilderStage},
    rlc::{circuit::RlcCircuitParams, virtual_region::RlcThreadBreakPoints},
    utils::component::{circuit::ComponentCircuitImpl, ComponentCircuit},
};

type ZkevmCircuit = ComponentCircuitImpl<Fr, ZkevmCircuitBuilder<Fr>, EmptyPromiseLoader<Fr>>;

#[tokio::main]
pub async fn main() {
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
        SuperCircuit_::<Fr>::build_from_witness_block(block).unwrap();

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

    println!("promise results fullfilled");

    println!("instance from super_circuit {:?}", instances);

    let public_instances = circuit.get_public_instances();
    // let instances = vec![
    //     instance[0].clone(),
    //     // instance[1].clone(),
    //     public_instances.into(),
    // ];
    instances.push(public_instances.into());

    // halo2_utils::compare::compare_all(&super_circuit, &circuit, Some(k));
    // halo2_utils::assignments::print_all(&circuit, Some(k), Some(100));
    println!("running circuit");
    let prover = MockProver::run(k, &circuit, instances).unwrap();
    println!("verifying constraints");
    prover.assert_satisfied_par();
    println!("success!");
}
