use super::*;

use plonky2::{field::types::Field, iop::target::Target, plonk::circuit_builder::CircuitBuilder};

/// Stores the information of a product of consecutive numbers embedded in a circuit.
/// The `first_factor` field identifies the position of the first number in the product,
/// let's call that number `n`. The `n_factors` number specifies how long is the product
/// being computed, i.e.
/// `n * (n + 1) * ... * (n + n_factors - 1)`
/// or more formally
/// `(n .. n + n_factors).fold(1, |prod, factor| prod * factor)`.
/// The `product_targets` field keeps track of all the targets that were created to
/// build the total product. more precisely, for every valid index `idx`, it holds
/// `product_targets[i] == (n .. n + idx).fold(1, |prod, factor| prod * factor)`
/// or equivalently, `product_targets[0] = 1`, and for every valid `idx` it holds
/// `product_targets[idx + 1] == product_targets[idx] * (n + idx)`
pub struct ConsecutiveProduct {
    n_factors: usize,
    first_factor: Target,
    product_targets: Vec<Target>,
}

fn build_prod_subcircuit(
    circuit_builder: &mut CircuitBuilder<BaseField, D>,
    first_factor: Target,
    n_factors: usize,
) -> Vec<Target> {
    let mut next_factor = first_factor;
    let mut product = circuit_builder.one();

    let mut prod_targets = Vec::with_capacity(n_factors + 1);
    prod_targets.push(product);

    for _ in 0..n_factors - 1 {
        product = circuit_builder.mul(product, next_factor);
        prod_targets.push(product);
        next_factor = circuit_builder.add_const(next_factor, BaseField::ONE);
    }
    product = circuit_builder.mul(product, next_factor);
    prod_targets.push(product);
    prod_targets
}

impl ConsecutiveProduct {
    /// Given a mutable reference to a `CircuitBuilder` instance, and given an
    /// `n_factors` number, this function adds a subcircuit that encodes the product
    /// of `n_factors` consecutive numbers. The first factor to be used in the product
    /// is encoded in a new virtual target stored in the `first_factor` field of the
    /// returned `ConsecutiveProduct` instance, and must later be connected to
    /// a concrete value.
    pub fn new(circuit_builder: &mut CircuitBuilder<BaseField, D>, n_factors: usize) -> Self {
        let private_input = circuit_builder.add_virtual_target();

        let product_targets = build_prod_subcircuit(circuit_builder, private_input, n_factors);

        Self {
            n_factors,
            first_factor: private_input,
            product_targets,
        }
    }

    /// Grants read access to `self.first_factor`
    pub fn first_factor_target(&self) -> Target {
        self.first_factor
    }

    /// A utility function to clone `self.product_targets`
    pub fn clone_product_targets(&self) -> Vec<Target> {
        self.product_targets.clone()
    }

    /// Grants read access to `self.product_targets[n_factors]`
    ///
    /// The output is the target where the number
    /// ``` text
    /// self.first_factor * (self.first_factor + 1) * ... * (self.first_factor + n_factors - 1)
    /// ```
    /// or more formally the number
    /// ``` text
    /// (self.first_factor .. self.first_factor + n_factors).fold(
    ///     self.first_factor,
    ///     |prod, factor| prod * factor
    /// )
    /// ```
    /// is stored.
    pub fn partial_product_target(&self, n_factors: usize) -> Option<Target> {
        self.product_targets.get(n_factors).map(|t| t.clone())
    }

    /// Outputs the position where the circuit stores the product
    ///
    /// ``` text
    /// self.first_factor * (self.first_factor + 1) * ... * (self.first_factor + self.n_factors - 1)
    /// ```
    ///
    /// or more formally
    ///
    /// ``` text
    /// (self.first_factor .. self.first_factor + self.n_factors).fold(
    ///     self.first_factor,
    ///     |prod, factor| prod * factor
    /// )
    /// ```
    ///
    /// Equivalent to `self.partial_product_target(self.n_factors)`.
    pub fn final_product_target(&self) -> Target {
        self.product_targets
            .last()
            .expect("This vector is never empty")
            .clone()
    }

    pub fn n_factors(&self) -> usize {
        self.n_factors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plonky2::{
        field::types::Field,
        iop::witness::{PartialWitness, WitnessWrite},
        plonk::{circuit_builder::CircuitBuilder, circuit_data::CircuitConfig},
    };

    #[test]
    fn test_prod_1000() {
        const N_FACTORS: usize = 1000;
        const N_TESTS: usize = 10;

        fn product_direct_computation(first_factor: BaseField) -> BaseField {
            (0..N_FACTORS)
                .fold((BaseField::ONE, first_factor), |(prod, factor), _| {
                    (prod * factor, factor + BaseField::ONE)
                })
                .0
        }

        let circuit_config = CircuitConfig::standard_recursion_zk_config();
        let mut circuit_builder = CircuitBuilder::<BaseField, D>::new(circuit_config);
        let prod_targets = ConsecutiveProduct::new(&mut circuit_builder, N_FACTORS);

        circuit_builder.register_public_input(prod_targets.final_product_target());

        let prod_circuit = circuit_builder.build::<PGConfig>();

        for _ in 0..N_TESTS {
            let first_factor = BaseField::from_canonical_u16(rand::random());

            let product_direct_computation = product_direct_computation(first_factor);

            let proof = prod_circuit
                .prove({
                    let mut witness = PartialWitness::new();
                    witness.set_target(prod_targets.first_factor_target(), first_factor);
                    witness
                })
                .expect("proof generation goes wrong");

            assert_eq!(proof.public_inputs[0], product_direct_computation);
        }
    }
}
