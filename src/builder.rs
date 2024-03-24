/// Zkevm Circuit
///
/// Prove a block using Axiom Component Framework
///
use axiom_eth::{
    halo2_base::AssignedValue,
    halo2_proofs::{
        circuit::Layouter,
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
            types::{EmptyComponentType, LogicalEmpty},
        },
    },
    // Field,
};

// use bus_mapping::circuit_input_builder::{FeatureConfig, FixedCParams};
use eth_types::Field;
use halo2_proofs::circuit::Value;
use zkevm_circuits::{
    super_circuit::{SuperCircuit as SuperCircuitBase, SuperCircuitConfig},
    util::Challenges,
};

use crate::init_state::{InitState, InitStateTable};

pub const MAX_TXS: usize = 1;
pub const MAX_CALLDATA: usize = 256;
pub const MAX_INNER_BLOCKS: usize = 1;
pub const TEST_MOCK_RANDOMNESS: u64 = 0x100;

pub const MOCK_CHAIN_ID: u64 = 1338;
// const MOCK_DIFFICULTY: Word = Word::from(0x200000u64);

pub type SuperCircuit<F> =
    SuperCircuitBase<F, MAX_TXS, MAX_CALLDATA, MAX_INNER_BLOCKS, TEST_MOCK_RANDOMNESS>;

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
    pub super_circuit: Option<SuperCircuit<F>>,
    pub init_state: Option<InitState<F>>,
}

impl<F: Field> DummyFrom<ZkevmCircuitParams> for ZkevmCircuitInput<F> {
    fn dummy_from(_seed: ZkevmCircuitParams) -> Self {
        ZkevmCircuitInput {
            super_circuit: None,
            init_state: None,
        }
    }
}

// Raw halo2 configuration
#[derive(Clone)]
pub struct ZkevmCircuitConfig {
    super_circuit: (SuperCircuitConfig<Fr>, Challenges),
    init_state_table: InitStateTable<Fr>,
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
                init_state: None,
            },
        }
    }

    fn get_params(&self) -> Self::Params {
        ZkevmCircuitParams
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<Fr>,
        params: Self::Params,
    ) -> Self::Config {
        ZkevmCircuitConfig {
            super_circuit: SuperCircuit::<Fr>::configure(meta),
            init_state_table: InitStateTable::<Fr>::construct(meta),
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
        self.input = input;
        Ok(())
    }

    fn virtual_assign_phase0(
        &mut self,
        builder: &mut RlcCircuitBuilder<Fr>,
        promise_caller: PromiseCaller<Fr>,
    ) -> CoreBuilderOutput<Fr, Self::CompType> {
        println!("virtual_assign_phase0");

        let ctx = builder.base.main(0);

        let account_calls = self
            .input
            .init_state
            .as_ref()
            .unwrap()
            .make_account_promise_calls(ctx, &promise_caller);
        let storage_calls = self
            .input
            .init_state
            .as_ref()
            .unwrap()
            .make_storage_promise_calls(ctx, &promise_caller);

        CoreBuilderOutput {
            public_instances: vec![],
            virtual_table: vec![],
            logical_results: vec![],
        }
    }

    fn raw_synthesize_phase0(&mut self, config: &Self::Config, layouter: &mut impl Layouter<Fr>) {
        println!("raw_synthesize_phase0");

        let assigned_table = config.init_state_table.load(
            self.input.init_state.as_ref().unwrap(),
            layouter,
            Value::known(Fr::zero()), // TODO use correct randomness here
        );

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
