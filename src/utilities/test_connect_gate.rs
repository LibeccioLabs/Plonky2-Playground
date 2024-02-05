use plonky2::{
    field::extension::Extendable,
    gates::gate::Gate,
    hash::hash_types::RichField,
    iop::{
        ext_target::ExtensionTarget,
        generator::{WitnessGenerator, WitnessGeneratorRef},
        target::Target,
    },
    plonk::{
        circuit_builder::CircuitBuilder,
        circuit_data::CommonCircuitData,
        vars::{EvaluationTargets, EvaluationVars},
    },
    util::serialization::{Buffer, IoResult},
};

#[cfg(test)]
use plonky2::{
    field::types::Field,
    iop::witness::{PartialWitness, WitnessWrite},
    plonk::{
        circuit_data::CircuitConfig,
        config::{GenericConfig, PoseidonGoldilocksConfig},
    },
};

/// When one wants to provide invalid witness values to some circuit,
/// cells that were made equal with PLONK's permutation argument,
/// i.e. using the function
/// `plonky2::plonk::circuit_builder::CircuitBuilder::connect`,
/// trigger a panic in the prover.
///
/// To address this problem, we introduce a gate that checks for equality in
/// a less optimized way, using the two values one wants to prove equal as
/// inputs in an arithmetic gate set to perform subtraction, and using the
/// output of the subtraction as a constraint to be proven equal to zero.
///
/// Using this gate instead of `connect` allows the circuit to preserve
/// the logic structure, and avoids a panic during the proving process.
#[derive(Debug, Clone, Copy)]
pub struct TestEq<const N_OPS: usize>;

impl<F: RichField + Extendable<D>, const D: usize, const N_OPS: usize> Gate<F, D>
    for TestEq<N_OPS>
{
    fn id(&self) -> String {
        format!("{self:?}")
    }

    fn serialize(
        &self,
        _dst: &mut Vec<u8>,
        _common_data: &CommonCircuitData<F, D>,
    ) -> IoResult<()> {
        Ok(())
    }

    fn num_wires(&self) -> usize {
        2 * N_OPS
    }

    fn num_constants(&self) -> usize {
        0
    }

    fn num_constraints(&self) -> usize {
        N_OPS
    }

    fn degree(&self) -> usize {
        1
    }

    fn deserialize(_src: &mut Buffer, _common_data: &CommonCircuitData<F, D>) -> IoResult<Self>
    where
        Self: Sized,
    {
        Ok(Self)
    }

    fn eval_unfiltered(&self, vars: EvaluationVars<F, D>) -> Vec<<F as Extendable<D>>::Extension> {
        Vec::from_iter(
            (0..N_OPS).map(|idx| vars.local_wires[2 * idx] - vars.local_wires[2 * idx + 1]),
        )
    }

    fn eval_unfiltered_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: EvaluationTargets<D>,
    ) -> Vec<ExtensionTarget<D>> {
        Vec::from_iter((0..N_OPS).map(|idx| {
            builder.sub_extension(vars.local_wires[2 * idx], vars.local_wires[2 * idx + 1])
        }))
    }

    fn generators(
        &self,
        _row: usize,
        _local_constants: &[F],
    ) -> Vec<plonky2::iop::generator::WitnessGeneratorRef<F, D>> {
        Vec::from_iter((0..N_OPS).map(|_| WitnessGeneratorRef(Box::new(NoOpGenerator))))
    }
}

#[derive(Debug, Clone, Copy)]
struct NoOpGenerator;

impl<Field: RichField + Extendable<D>, const D: usize> WitnessGenerator<Field, D>
    for NoOpGenerator
{
    fn id(&self) -> String {
        format!("{self:?}")
    }

    fn serialize(
        &self,
        _dst: &mut Vec<u8>,
        _common_data: &CommonCircuitData<Field, D>,
    ) -> IoResult<()> {
        Ok(())
    }

    fn deserialize(_src: &mut Buffer, _common_data: &CommonCircuitData<Field, D>) -> IoResult<Self>
    where
        Self: Sized,
    {
        Ok(NoOpGenerator)
    }

    fn watch_list(&self) -> Vec<Target> {
        vec![]
    }
    fn run(
        &self,
        _witness: &plonky2::iop::witness::PartitionWitness<Field>,
        _out_buffer: &mut plonky2::iop::generator::GeneratedValues<Field>,
    ) -> bool {
        true
    }
}

impl<const N_OPS: usize> TestEq<N_OPS> {
    pub fn connect<Field: RichField + Extendable<D>, const D: usize>(
        builder: &mut CircuitBuilder<Field, D>,
        lhs: Target,
        rhs: Target,
    ) {
        let (gate_row, op) = builder.find_slot(Self, &[], &[]);

        builder.connect(lhs, Target::wire(gate_row, 2 * op));
        builder.connect(rhs, Target::wire(gate_row, 2 * op + 1));
    }
}

#[test]
fn test_test_eq_gate() {
    const D: usize = 2;
    type BaseField = <PoseidonGoldilocksConfig as GenericConfig<D>>::F;

    let mut builder =
        CircuitBuilder::<BaseField, D>::new(CircuitConfig::standard_recursion_zk_config());

    let gate_row = builder.add_gate(TestEq::<1>, vec![]);

    let circuit = builder.build::<PoseidonGoldilocksConfig>();

    let mut correct_witness = PartialWitness::new();

    correct_witness.set_target(Target::wire(gate_row, 0), BaseField::ONE);

    let mut incorrect_witness = correct_witness.clone();

    incorrect_witness.set_target(Target::wire(gate_row, 1), BaseField::ZERO);
    correct_witness.set_target(Target::wire(gate_row, 1), BaseField::ONE);

    let incorrect_proof = circuit.prove(incorrect_witness);
    let correct_proof = circuit.prove(correct_witness);

    match incorrect_proof {
        Ok(proof) => match circuit.verify(proof) {
            Err(err) => {
                println!("The invalid proof is correctly identified as invalid because\n{err:?}");
            }
            _ => panic!("invalid proof is accepted!"),
        },
        _ => panic!("proof generation with invalid witness fails."),
    }

    match correct_proof {
        Ok(proof) => match circuit.verify(proof) {
            Ok(()) => {
                println!("The valid proof is correctly identified as valid")
            }
            _ => panic!("valid proof is rejected!"),
        },
        _ => panic!("proof generation with valid witness fails"),
    }
}
