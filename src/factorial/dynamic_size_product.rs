use super::*;

use plonky2::{
    field::types::Field,
    iop::{
        target::Target,
        witness::{PartialWitness, WitnessWrite},
    },
    plonk::{
        circuit_builder::CircuitBuilder,
        circuit_data::{CircuitConfig, CircuitData},
    },
};

use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};

use std::sync::OnceLock;

/// This struct holds all the relevant data for a circuit that is capable
/// of computing the product of arbitrarily many consecutive numbers.
///
/// The utility functions `prove` and `verify` are provided to
/// abstract the details of the workflow needed to achieve this functionality.
///
/// Nonetheless, a high level overview is: first there is the computation
/// of a product of a fixed amount of consecutive numbers. Then, this
/// computation is embedded into a cyclically recursive circuit, so that
/// the same computation can be aggregated over and over until the desired
/// number of factors has been multiplied together.
/// Then, the resulting proof is fed to a further circuit that only exposes
/// the desired inputs, which in our case are the number of factors and
/// the final product.
///
/// The verifier of this final proof reads two public inputs, let's name them
/// `n_factors` and `product`, and is convinced that the prover knows a number
/// `k` such that
///
/// ``` text
/// k * (k + 1) * ... * (k + n_factors - 1) == product
/// ```
///
/// or more formally
///
/// ``` text
/// (k .. k + n_factors).fold(1, |prod, factor| {prod * factor}) == product
/// ```
pub struct RecursiveProdCircuitData {
    recursive_circuit_data: CircuitData<BaseField, PGConfig, D>,
    n_factors_target: Target,
    first_chunk_factor_target: Target,
    remaining_factors_public_input_idx: usize,
    n_factors_public_input_idx: usize,
    product_after_chunk_public_input_idx: usize,
    first_factor_after_chunk_public_input_idx: usize,
    cyclic_proof_target: ProofWithPublicInputsTarget<D>,
}

static PRODUCT_CIRCUIT_DATA: OnceLock<(
    CircuitData<BaseField, PGConfig, D>,
    ProofWithPublicInputsTarget<D>,
)> = OnceLock::new();
impl RecursiveProdCircuitData {
    fn product_circuit_data(
        &self,
    ) -> &'static (
        CircuitData<BaseField, PGConfig, D>,
        ProofWithPublicInputsTarget<D>,
    ) {
        PRODUCT_CIRCUIT_DATA.get_or_init(|| self.product_circuit_data_constructor())
    }

    fn product_circuit_data_constructor(
        &self,
    ) -> (
        CircuitData<BaseField, PGConfig, D>,
        ProofWithPublicInputsTarget<D>,
    ) {
        let circuit_config = CircuitConfig::standard_recursion_zk_config();
        let mut circuit_builder = CircuitBuilder::<BaseField, D>::new(circuit_config);

        let inner_proof_target =
            circuit_builder.add_virtual_proof_with_pis(&self.recursive_circuit_data.common);

        let inner_verifier_target =
            circuit_builder.constant_verifier_data(&self.recursive_circuit_data.verifier_only);

        circuit_builder.verify_proof::<PGConfig>(
            &inner_proof_target,
            &inner_verifier_target,
            &self.recursive_circuit_data.common,
        );

        // here we make sure that the recursive proof's data is correctly matched
        // by connecting the verifier data encoded in the public input of the
        // recursive proof to the constant verifier data we know to be correct.
        let public_input_inner_verifier_target =
            crate::utilities::copy_of_private_plonky2_functions::from_slice(
                &inner_proof_target.public_inputs,
                &self.recursive_circuit_data.common,
            )
            .expect("inner proof has enough public inputs");
        circuit_builder
            .connect_verifier_data(&inner_verifier_target, &public_input_inner_verifier_target);

        // we register two public inputs: one is the number of factors in the product,
        // and the other is the product itself.
        // Those numbers are available in the public input targets of
        // the inner proof.
        circuit_builder.register_public_input(
            inner_proof_target.public_inputs[self.n_factors_public_input_idx],
        );
        circuit_builder.register_public_input(
            inner_proof_target.public_inputs[self.product_after_chunk_public_input_idx],
        );

        (circuit_builder.build(), inner_proof_target)
    }

    /// Computes
    ///  
    /// ``` text
    /// starting_product * first_factor * (first_factor + 1) * ... * (first_factor + n_factors - 1)
    /// ```
    ///
    /// and creates a proof that exposes `n_factors` and the computed product
    /// as public inputs.
    pub fn prove(
        &self,
        n_factors: usize,
        first_factor: BaseField,
        starting_product: BaseField,
    ) -> ProofWithPublicInputs<BaseField, PGConfig, D> {
        let n_factors = BaseField::from_canonical_usize(n_factors);

        let mut witness = PartialWitness::new();

        witness.set_target(self.n_factors_target, n_factors);
        witness.set_target(self.first_chunk_factor_target, first_factor);

        let base_case_public_inputs = [
            (self.n_factors_public_input_idx, n_factors),
            (self.remaining_factors_public_input_idx, n_factors),
            (self.product_after_chunk_public_input_idx, starting_product),
            (self.first_factor_after_chunk_public_input_idx, first_factor),
        ]
        .into();

        witness.set_proof_with_pis_target(
            &self.cyclic_proof_target,
            &plonky2::recursion::dummy_circuit::cyclic_base_proof(
                &self.recursive_circuit_data.common,
                &self.recursive_circuit_data.verifier_only,
                base_case_public_inputs,
            ),
        );

        let mut proof = self
            .recursive_circuit_data
            .prove(witness)
            .expect("proof generation goes wrong");
        while proof.public_inputs[self.remaining_factors_public_input_idx] != BaseField::ZERO {
            witness = PartialWitness::new();

            witness.set_proof_with_pis_target(&self.cyclic_proof_target, &proof);

            proof = self
                .recursive_circuit_data
                .prove(witness)
                .expect("proof generation goes wrong");
        }

        let (product_circuit, proof_target) = self.product_circuit_data();

        let mut witness = PartialWitness::new();
        witness.set_proof_with_pis_target(&proof_target, &proof);
        product_circuit
            .prove(witness)
            .expect("proof generation fails")
    }

    /// Upon successful verification of the proof, the verifier knows that,
    /// naming `n_factors` and `product` the public inputs of the proof,
    /// the prover knows a number `k` such that
    ///
    /// ``` text
    /// k * (k + 1) * ... * (k + n_factors - 1) == product
    /// ```
    ///
    /// or more formally
    ///
    /// ``` text
    /// (k .. k + n_factors).fold(1, |prod, factor| {prod * factor}) == product
    /// ```
    pub fn verify(
        &self,
        proof_with_public_inputs: ProofWithPublicInputs<BaseField, PGConfig, 2>,
    ) -> Result<(), anyhow::Error> {
        self.product_circuit_data()
            .0
            .verify(proof_with_public_inputs)
    }
}

static RECURSIVE_PROD_CIRCUIT: OnceLock<RecursiveProdCircuitData> = OnceLock::new();
pub fn recursive_product_circuit() -> &'static RecursiveProdCircuitData {
    // Setting up a cyclic recursion requires a bootstrapping procedure. The number of steps
    // needed for this procedure was determined experimentally.
    // At the moment I have no formal explanation of the reason why this works.
    const N_BOOTSTRAP_STEPS: usize = 2;
    RECURSIVE_PROD_CIRCUIT
        .get_or_init(|| build_recursive_product_circuit(N_BOOTSTRAP_STEPS, N_BOOTSTRAP_STEPS))
}

fn build_recursive_product_circuit(
    total_bootstrap_steps: usize,
    remaining_bootstrap_steps: usize,
) -> RecursiveProdCircuitData {
    const MAX_N_FACTORS_BITS: usize = 32;
    const CHUNK_SIZE_LOG: usize = 5;
    const CHUNK_SIZE: usize = 1 << CHUNK_SIZE_LOG;

    let circuit_config = CircuitConfig::standard_recursion_config();
    let mut circuit_builder = CircuitBuilder::<BaseField, D>::new(circuit_config.clone());

    // Getting constant valued targets requires a mutable access to
    // circuit_builder, so we declare the constants we need beforehand.
    let const_chunk_size = circuit_builder.constant(BaseField::from_canonical_usize(CHUNK_SIZE));
    let const_zero = circuit_builder.zero();
    let const_one = circuit_builder.one();

    let product_before_chunk = circuit_builder.add_virtual_target();
    let consecutive_product_target = ConsecutiveProduct::new(&mut circuit_builder, CHUNK_SIZE);
    let first_chunk_factor = consecutive_product_target.first_factor_target();

    let n_factors = circuit_builder.add_virtual_target();
    let remaining_factors_before_chunk = circuit_builder.add_virtual_target();

    // Now we check how many factors we should integrate into the product
    // after `min(remaining_factors_before_chunk, CHUNK_SIZE)` many
    // factors have been included.
    circuit_builder.range_check(remaining_factors_before_chunk, MAX_N_FACTORS_BITS);
    let (_, chunk_size_multiple) = circuit_builder.split_low_high(
        remaining_factors_before_chunk,
        CHUNK_SIZE_LOG,
        MAX_N_FACTORS_BITS,
    );
    let no_more_resursion_needed = circuit_builder.is_equal(chunk_size_multiple, const_zero);
    let remaining_factors_after_chunk = {
        let sub = circuit_builder.sub(remaining_factors_before_chunk, const_chunk_size);
        circuit_builder.select(no_more_resursion_needed, const_zero, sub)
    };

    // `number_of_new_factors = min(
    //      remaining_factors_before_chunk,
    //      CHUNK_SIZE
    // )`
    // because, if `remaining_factors_before_chunk >= CHUNK_SIZE`, then
    // `remaining_factors_after_chunk =
    //      remaining_factors_before_chunk - CHUNK_SIZE`
    // otherwise, `remaining_factors_after_chunk = 0`.
    let number_of_chunk_factors = circuit_builder.sub(
        remaining_factors_before_chunk,
        remaining_factors_after_chunk,
    );

    let product_after_chunk = {
        // the product targets we are selecting on are, for some number `n`,
        // where `n` is stored in `first_chunk_factor`,
        // [
        //     1,
        //     n,
        //     n * (n + 1),
        //     n * (n + 1) * (n + 2),
        //     ...,
        //     n * (n + 1) * ... * (n + CHUNK_SIZE - 1)
        // ]
        // so, selecting its `number_of_new_factors`-th value amounts to
        // selecting the product
        // `n * (n + 1) * ... * (n + number_of_chunk_factors - 1)`
        let mut cumulated_product_targets = consecutive_product_target.clone_product_targets();
        cumulated_product_targets.resize(2 * CHUNK_SIZE, const_zero);
        let chunk_product =
            circuit_builder.random_access(number_of_chunk_factors, cumulated_product_targets);

        circuit_builder.mul(product_before_chunk, chunk_product)
    };

    let first_factor_after_chunk = circuit_builder.add(first_chunk_factor, number_of_chunk_factors);

    // Here we take care of the recursive proof verification.

    // We detect if this is the base case. In the base case,
    // we did not multiply anything yet, and we don't have to verify
    // any recursive proof. We only have to make sure that the
    // cumulated product is set to 1.
    //
    // When we are not in the base case, we have to check that the
    // initial values coincide with the ones provided in the public
    // inputs of the recursive proof.
    let is_base_case = circuit_builder.is_equal(n_factors, remaining_factors_before_chunk);
    let is_not_base_case = circuit_builder.not(is_base_case);

    // if `is_base_case`, then we want that `product_before_chunk == 1`.
    let initial_product_equal_one = circuit_builder.is_equal(product_before_chunk, const_one);
    circuit_builder.or(is_not_base_case, initial_product_equal_one);

    // We register public inputs in the circuit, all in one go.
    // This allows us to better track the serialization order of
    // the public inputs of the circuit.
    let public_inputs_vector = vec![
        n_factors,
        remaining_factors_before_chunk,
        remaining_factors_after_chunk,
        first_chunk_factor,
        first_factor_after_chunk,
        product_before_chunk,
        product_after_chunk,
    ];
    circuit_builder.register_public_inputs(&public_inputs_vector);

    // To produce the circuit data used to build the proof target
    // for the cyclic proof, we need to employ a bootstrapping process.
    let proof_target_circuit_data = if remaining_bootstrap_steps == 0 {
        let mut circuit_builder = CircuitBuilder::<BaseField, D>::new(circuit_config.clone());

        let num_cyclic_proof_public_inputs = 4 + 4 * circuit_config.fri_config.num_cap_elements();

        for _ in 0..num_cyclic_proof_public_inputs + public_inputs_vector.len() {
            circuit_builder.add_virtual_public_input();
        }

        circuit_builder.build::<PGConfig>()
    }
    // If we are not producing dummy circuit data, we
    // compute the structure of the inner proof using the data
    // produced by a recursive call to this function.
    else {
        build_recursive_product_circuit(total_bootstrap_steps, remaining_bootstrap_steps - 1)
            .recursive_circuit_data
    };

    let cyclic_proof_target =
        circuit_builder.add_virtual_proof_with_pis(&proof_target_circuit_data.common);

    if total_bootstrap_steps != remaining_bootstrap_steps {
        //This block mocks a call to
        // circuit_builder.add_verifier_data_public_inputs()
        //
        // but it does not set the inner logic in circuit_builder
        // that would later check if the data of the mock verifier
        // actually matches with the verifier data of itself.
        let mock_verifier_data =
            circuit_builder.add_virtual_verifier_data(circuit_builder.config.fri_config.cap_height);
        // The verifier data are public inputs.
        circuit_builder.register_public_inputs(&mock_verifier_data.circuit_digest.elements);
        for i in 0..circuit_builder.config.fri_config.num_cap_elements() {
            circuit_builder
                .register_public_inputs(&mock_verifier_data.constants_sigmas_cap.0[i].elements);
        }

        // This call mocks the call to
        // circuit_builder.conditionally_verify)cyclic_proof_or_dummy()
        // but it does not require a previous call to
        // circuit_builder.add_verifier_data_public_inputs()
        circuit_builder
            .conditionally_verify_proof_or_dummy::<PGConfig>(
                is_not_base_case,
                &cyclic_proof_target,
                &mock_verifier_data,
                &proof_target_circuit_data.common,
            )
            .expect("Insertion of mock circuit verification goes wrong");
    } else {
        // This function adds public inputs that correspond to the
        // verifier data for this circuit.
        circuit_builder.add_verifier_data_public_inputs();

        // We verify the recursive proof if this is not the base case.
        //
        // WARNING: this function introduces public inputs that will be used to
        // encode the verification key for cyclic proof verification, and
        // no public inputs should be registered after this function
        // is called. This is because the function
        // `CircuitBuilder::conditionally_verify_cyclic_proof`
        // is called during the execution of
        // `CircuitBuilder::conditionally_verify_cyclic_proof_or_dummy`
        circuit_builder
            .conditionally_verify_cyclic_proof_or_dummy::<PGConfig>(
                is_not_base_case,
                &cyclic_proof_target,
                &proof_target_circuit_data.common,
            )
            .expect("cannot insert cyclic proof verification in circuit.");
    }
    /// We will have to provide the position of some of the public input values
    /// for external entities. This function ensures that we cannot make indexing
    /// errors if the public inputs, or their ordering, change.
    fn find_public_input_idx(public_inputs_vector: &Vec<Target>, target: Target) -> usize {
        public_inputs_vector
            .iter()
            .enumerate()
            .find(|(_idx, &t)| t == target)
            .expect("the target should be in the vector")
            .0
    }

    /// We will have to connect some of the public inputs of the cyclic
    /// proof with the public inputs of the outer proof. For this reason,
    /// we need to know the position we can find those public inputs in.
    ///
    /// This function ensures that we cannot make indexing errors
    /// if the public inputs, or their order, change.
    fn find_inner_public_input(
        public_inputs_vector: &Vec<Target>,
        cyclic_proof: &ProofWithPublicInputsTarget<2>,
        target: Target,
    ) -> Target {
        cyclic_proof.public_inputs[find_public_input_idx(public_inputs_vector, target)]
    }

    // If this is not the base case, we impose the appropriate equality
    // constraints between the public inputs of the recursive proof
    // and the public inputs of this proof.
    for (outer_target, inner_target) in [
        (
            remaining_factors_before_chunk,
            remaining_factors_after_chunk,
        ),
        (first_chunk_factor, first_factor_after_chunk),
        (product_before_chunk, product_after_chunk),
        (n_factors, n_factors),
    ] {
        circuit_builder.connect(
            find_inner_public_input(&public_inputs_vector, &cyclic_proof_target, inner_target),
            outer_target,
        );
    }

    let recursive_circuit_data = circuit_builder.build();

    let remaining_factors_public_input_idx =
        find_public_input_idx(&public_inputs_vector, remaining_factors_after_chunk);
    let n_factors_public_input_idx = find_public_input_idx(&public_inputs_vector, n_factors);
    let product_after_chunk_public_input_idx =
        find_public_input_idx(&public_inputs_vector, product_after_chunk);
    let first_factor_after_chunk_public_input_idx =
        find_public_input_idx(&public_inputs_vector, first_factor_after_chunk);

    RecursiveProdCircuitData {
        recursive_circuit_data,
        n_factors_target: n_factors,
        first_chunk_factor_target: first_chunk_factor,
        remaining_factors_public_input_idx,
        n_factors_public_input_idx,
        product_after_chunk_public_input_idx,
        first_factor_after_chunk_public_input_idx,
        cyclic_proof_target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recursive_prod() {
        let recursive_product_circuit = recursive_product_circuit();

        let test_round_parameters = [(1, 5), (1, 10), (42, 130), (42, 420)];

        for (first_factor, n_factors) in test_round_parameters {
            let first_factor = BaseField::from_canonical_usize(first_factor);
            let proof = recursive_product_circuit.prove(n_factors, first_factor, BaseField::ONE);

            let expected_product = (0..n_factors)
                .fold(
                    (BaseField::ONE, first_factor),
                    |(prod, next_factor), _iter_nr| {
                        (prod * next_factor, next_factor + BaseField::ONE)
                    },
                )
                .0;

            assert_eq!(
                BaseField::from_canonical_usize(n_factors),
                proof.public_inputs[0]
            );
            assert_eq!(expected_product, proof.public_inputs[1]);

            recursive_product_circuit
                .verify(proof)
                .expect("proof verification goes wrong");
        }
    }
}
