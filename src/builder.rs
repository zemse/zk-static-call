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
use crate::{
    common::HiLoRlc,
    init_state::{
        account::{AxiomAccountPayload, Halo2AccountPayload},
        storage::{AxiomStoragePayload, Halo2StoragePayload},
        InitState, InitStateTable,
    },
};
use eth_types::Field;
use halo2_proofs::circuit::Value;
use zkevm_circuits::{
    super_circuit::{SuperCircuit as SuperCircuitBase, SuperCircuitConfig},
    util::Challenges,
};

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

#[allow(clippy::type_complexity)]
pub struct ZkevmCircuitBuilder<F: Field> {
    input: ZkevmCircuitInput<F>,
    axiom_payload: Option<(Vec<AxiomAccountPayload<F>>, Vec<AxiomStoragePayload<Fr>>)>,
    halo2_payload: Option<(Vec<Halo2AccountPayload<F>>, Vec<Halo2StoragePayload<Fr>>)>,
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
            axiom_payload: None,
            halo2_payload: None,
        }
    }

    fn get_params(&self) -> Self::Params {
        ZkevmCircuitParams
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<Fr>,
        params: Self::Params,
    ) -> Self::Config {
        let super_circuit = SuperCircuit::<Fr>::configure(meta);
        let init_state_table = InitStateTable::<Fr>::construct(meta);

        let rw_table = super_circuit.0.evm_circuit.rw_table;

        // TODO first make sure that RLC works
        // All Account and AccountStorage entries in RW table should exist in InitState table.
        // meta.lookup_any("exhaustive init state", |meta| {
        //     // RW table
        //     let s = meta.query_advice(rw_table.is_state, Rotation::cur());
        //     let address_rw = meta.query_advice(rw_table.address, Rotation::cur());
        //     let field_tag_rw = meta.query_advice(rw_table.field_tag, Rotation::cur());
        //     let storage_key_rw = meta.query_advice(rw_table.storage_key, Rotation::cur());
        //     let value_rw = meta.query_advice(rw_table.value, Rotation::cur());

        //     // InitState table
        //     let address_is = meta.query_advice(init_state_table.address, Rotation::cur());
        //     let field_tag_is = meta.query_advice(init_state_table.field_tag, Rotation::cur());
        //     let storage_key_is =
        //         meta.query_advice(init_state_table.storage_key.rlc, Rotation::cur());
        //     let value_is = meta.query_advice(init_state_table.value.rlc, Rotation::cur());

        //     vec![
        //         (s.expr() * address_rw, address_is),
        //         (s.expr() * field_tag_rw, field_tag_is),
        //         (s.expr() * storage_key_rw, storage_key_is),
        //         (s * value_rw, value_is),
        //     ]
        // });

        ZkevmCircuitConfig {
            super_circuit,
            init_state_table,
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

        let account_calls: Vec<crate::init_state::account::AccountPayload<AssignedValue<Fr>>> =
            self.input
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

        self.axiom_payload = Some((account_calls, storage_calls));

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

        self.halo2_payload = Some(assigned_table);

        self.input
            .super_circuit
            .as_ref()
            .unwrap()
            .synthesize_2(config.super_circuit.clone(), layouter)
            .unwrap();
    }

    fn virtual_assign_phase1(&mut self, builder: &mut RlcCircuitBuilder<Fr>) {
        println!("virtual_assign_phase1");

        if self.axiom_payload.is_none() || self.axiom_payload.is_none() {
            return;
        }

        let axiom_payload = self.axiom_payload.as_ref().unwrap();
        let halo2_payload = self.halo2_payload.as_ref().unwrap();

        let cm = builder.copy_manager().clone();
        let mut cm = cm.lock().unwrap();

        let ctx = builder.base.main(0);

        // constrain account values in init state to be the output of axiom promise calls
        for (axiom, halo2) in axiom_payload.0.iter().zip(halo2_payload.0.iter()) {
            let loaded = AxiomAccountPayload {
                block_number: cm.load_external_assigned(halo2.block_number),
                address: cm.load_external_assigned(halo2.address),
                field_idx: cm.load_external_assigned(halo2.field_idx),
                value: HiLoRlc::from(
                    cm.load_external_assigned(halo2.value.hi()),
                    cm.load_external_assigned(halo2.value.lo()),
                    cm.load_external_assigned(halo2.value.rlc()),
                ),
            };

            ctx.constrain_equal(&loaded.block_number, &axiom.block_number);
            ctx.constrain_equal(&loaded.address, &axiom.address);
            ctx.constrain_equal(&loaded.field_idx, &axiom.field_idx);

            ctx.constrain_equal(&loaded.value.hi(), &axiom.value.hi());
            ctx.constrain_equal(&loaded.value.lo(), &axiom.value.lo());
            ctx.constrain_equal(&loaded.value.rlc(), &axiom.value.rlc());
            println!("acc");
        }

        // constrain storage values in init state to be the output of axiom promise calls
        for (axiom, halo2) in axiom_payload.1.iter().zip(halo2_payload.1.iter()) {
            let loaded = AxiomStoragePayload {
                block_number: cm.load_external_assigned(halo2.block_number),
                address: cm.load_external_assigned(halo2.address),
                slot: HiLoRlc::from(
                    cm.load_external_assigned(halo2.slot.hi()),
                    cm.load_external_assigned(halo2.slot.lo()),
                    cm.load_external_assigned(halo2.slot.rlc()),
                ),
                value: HiLoRlc::from(
                    cm.load_external_assigned(halo2.value.hi()),
                    cm.load_external_assigned(halo2.value.lo()),
                    cm.load_external_assigned(halo2.value.rlc()),
                ),
            };

            ctx.constrain_equal(&loaded.block_number, &axiom.block_number);
            ctx.constrain_equal(&loaded.address, &axiom.address);

            ctx.constrain_equal(&loaded.slot.hi(), &axiom.slot.hi());
            ctx.constrain_equal(&loaded.slot.lo(), &axiom.slot.lo());
            ctx.constrain_equal(&loaded.slot.rlc(), &axiom.value.rlc());

            ctx.constrain_equal(&loaded.value.hi(), &axiom.value.hi());
            ctx.constrain_equal(&loaded.value.lo(), &axiom.value.lo());
            ctx.constrain_equal(&loaded.value.rlc(), &axiom.value.rlc());
            println!("store");
        }
        println!("done");
    }

    fn raw_synthesize_phase1(&mut self, _config: &Self::Config, _layouter: &mut impl Layouter<Fr>) {
        println!("raw_synthesize_phase1");
    }
}
