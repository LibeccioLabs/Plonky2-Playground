use plonky2::{field::types::Field, iop::witness::WitnessWrite};
use rand::distributions::Distribution;

/// Helper function to generate symbols and a list of problems
/// The return value is a tuple, laid out as
/// `(symbols, impl Iterator<Item = (solution, problem)>)`
///
/// The values provided are usize arrays. To use them in a
/// sudoku circuit, they have to be converted in Fp values.
fn numeric_setup_values(
    nr_random_masks_per_problem: usize,
) -> (
    [usize; 9],
    impl IntoIterator<Item = ([[usize; 9]; 9], [[usize; 9]; 9])>,
) {
    let symbols = core::array::from_fn(|n| n + 1);

    let grids = [
        [
            [2, 4, 9, 5, 3, 6, 1, 8, 7],
            [3, 5, 1, 2, 7, 8, 4, 9, 6],
            [6, 7, 8, 4, 9, 1, 5, 3, 2],
            [8, 9, 7, 1, 4, 5, 6, 2, 3],
            [4, 2, 3, 6, 8, 9, 7, 5, 1],
            [5, 1, 6, 7, 2, 3, 9, 4, 8],
            [1, 6, 2, 3, 5, 4, 8, 7, 9],
            [9, 3, 5, 8, 6, 7, 2, 1, 4],
            [7, 8, 4, 9, 1, 2, 3, 6, 5],
        ],
        [
            [1, 7, 5, 3, 4, 9, 8, 2, 6],
            [4, 2, 9, 8, 7, 6, 1, 5, 3],
            [3, 6, 8, 5, 1, 2, 9, 7, 4],
            [2, 8, 3, 7, 6, 1, 4, 9, 5],
            [7, 1, 6, 4, 9, 5, 2, 3, 8],
            [9, 5, 4, 2, 8, 3, 7, 6, 1],
            [8, 3, 7, 9, 5, 4, 6, 1, 2],
            [5, 9, 1, 6, 2, 8, 3, 4, 7],
            [6, 4, 2, 1, 3, 7, 5, 8, 9],
        ],
        [
            [1, 8, 5, 2, 7, 3, 6, 9, 4],
            [4, 2, 7, 8, 9, 6, 5, 3, 1],
            [3, 6, 9, 4, 1, 5, 7, 2, 8],
            [8, 1, 4, 3, 5, 2, 9, 7, 6],
            [6, 7, 3, 9, 4, 1, 8, 5, 2],
            [5, 9, 2, 7, 6, 8, 1, 4, 3],
            [9, 3, 1, 5, 8, 4, 2, 6, 7],
            [2, 5, 6, 1, 3, 7, 4, 8, 9],
            [7, 4, 8, 6, 2, 9, 3, 1, 5],
        ],
        [
            [1, 2, 3, 4, 5, 6, 7, 8, 9],
            [4, 5, 6, 7, 8, 9, 1, 2, 3],
            [7, 8, 9, 1, 2, 3, 4, 5, 6],
            [2, 1, 4, 3, 6, 5, 8, 9, 7],
            [3, 6, 5, 8, 9, 7, 2, 1, 4],
            [8, 9, 7, 2, 1, 4, 3, 6, 5],
            [5, 3, 1, 6, 4, 2, 9, 7, 8],
            [6, 4, 2, 9, 7, 8, 5, 3, 1],
            [9, 7, 8, 5, 3, 1, 6, 4, 2],
        ],
    ];

    let nr_grids = grids.len();
    // We transform the sudoku grids into an iterator of grids of field elements
    let grids_iter = (0..nr_grids * nr_random_masks_per_problem)
        .into_iter()
        .map(|_| rand::random::<[[bool; 9]; 9]>())
        .enumerate()
        .map(move |(idx, mask)| {
            let grid = grids[idx / nr_random_masks_per_problem];

            let masked_grid = core::array::from_fn(|col_idx| {
                core::array::from_fn(|row_idx| {
                    if mask[col_idx][row_idx] {
                        0
                    } else {
                        grid[col_idx][row_idx]
                    }
                })
            });
            (grid, masked_grid)
        });

    (symbols, grids_iter)
}

/// Tests that we are able to prove knowledge of valid solution to
/// sudoku problem instances.
#[test]
fn test_valid_sudoku_problems() {
    const NR_RANDOM_MASKS_PER_PROBLEM: usize = 4;

    const SIZE: usize = 9;
    const SIZE_SQRT: usize = 3;

    type PlonkConfig = plonky2::plonk::config::PoseidonGoldilocksConfig;
    const FIELD_EXTENSION_DEGREE: usize = 2;
    type BaseField =
        <PlonkConfig as plonky2::plonk::config::GenericConfig<FIELD_EXTENSION_DEGREE>>::F;

    let circuit_config =
        plonky2::plonk::circuit_data::CircuitConfig::standard_recursion_zk_config();

    let mut builder = plonky2::plonk::circuit_builder::CircuitBuilder::<
        BaseField,
        FIELD_EXTENSION_DEGREE,
    >::new(circuit_config);

    let sudoku_problem_target =
        super::SudokuCircuitBuilder::<SIZE, SIZE_SQRT>::add_proof_of_sudoku_solution(&mut builder)
            .expect("Circuit building goes wrong.");

    let circuit = builder.build::<PlonkConfig>();

    let (_symbols, sudoku_problem_instances) = numeric_setup_values(NR_RANDOM_MASKS_PER_PROBLEM);

    for (solution, problem) in sudoku_problem_instances {
        let mut witness = plonky2::iop::witness::PartialWitness::<BaseField>::new();

        super::SudokuWitnessBuilder::set_sudoku_witness(
            &mut witness,
            &sudoku_problem_target,
            problem,
            solution,
        );

        let proof = crate::time_it! {
            circuit.prove(witness).expect("proof generation goes wrong");
            "Proof generation takes {:?}"
        };

        println! {
            "Proof size is {}",
            proof.to_bytes().len()
        };

        crate::time_it! {
            circuit
                .verify(proof)
                .expect("Proof verification goes wrong");
            "Proof verification takes {:?}"
        }
    }
}

/// Tests that we are able to prove knowledge of valid solution to
/// sudoku problem instances.
///
/// This test is not really satisfactory because it fails at witness
/// generation stage.
/// We were not able to generate invalid witnesses to test how the prover
/// would react being given logically inconsistent input.
///
/// TODO: if we have time, try building a more satisfactory test.
#[test]
fn test_invalid_sudoku_problems() {
    const NR_RANDOM_MASKS_PER_PROBLEM: usize = 4;

    const SIZE: usize = 9;
    const SIZE_SQRT: usize = 3;

    type PlonkConfig = plonky2::plonk::config::PoseidonGoldilocksConfig;
    const FIELD_EXTENSION_DEGREE: usize = 2;
    type BaseField =
        <PlonkConfig as plonky2::plonk::config::GenericConfig<FIELD_EXTENSION_DEGREE>>::F;

    let circuit_config =
        plonky2::plonk::circuit_data::CircuitConfig::standard_recursion_zk_config();

    let uniform_size = rand::distributions::Uniform::new(0, SIZE);
    let uniform_size_minus_1 = rand::distributions::Uniform::new(1, SIZE);

    let (_symbols, sudoku_problem_instances) = numeric_setup_values(NR_RANDOM_MASKS_PER_PROBLEM);

    for (mut solution, problem) in sudoku_problem_instances {
        // We have to build the circuit from scratch at every iteration because
        // `plonky2::plonk::circuit_data::CircuitData` is not `Clone`,
        // and because its serialization methods require a lot of overhead to be used.
        let mut builder = plonky2::plonk::circuit_builder::CircuitBuilder::<
            BaseField,
            FIELD_EXTENSION_DEGREE,
        >::new(circuit_config.clone());

        let sudoku_problem_target =
            super::SudokuCircuitBuilder::<SIZE, SIZE_SQRT>::add_proof_of_sudoku_solution_no_panic(
                &mut builder,
            )
            .expect("Circuit building goes wrong.");

        let circuit = builder.build::<PlonkConfig>();

        // We corrupt the solution of this sudoku instance by
        // changing a random cell to a random invalid value.

        // We extract the coordinates of the cell to corrupt.
        let err_row_idx = uniform_size.sample(&mut rand::rngs::OsRng);
        let err_col_idx = uniform_size.sample(&mut rand::rngs::OsRng);

        // We change its value to any other valid symbol.
        // We assume that `_symbols = [1..=SIZE]`
        let old_value = solution[err_row_idx][err_col_idx];
        let new_value = uniform_size_minus_1.sample(&mut rand::rngs::OsRng);
        let new_value = if new_value < old_value {
            new_value
        } else {
            new_value + 1
        };
        solution[err_row_idx][err_col_idx] = new_value;

        println!("modified ({err_row_idx}, {err_col_idx})");
        println! {"["};
        for row in solution {
            println! {"  {:?}", row};
        }
        println! {"]"};

        // Now we perform the witness and proof building operations as usual,
        // expecting that the proof generation will fail.

        let mut witness = plonky2::iop::witness::PartialWitness::<BaseField>::new();

        witness.set_target_arr(
            &sudoku_problem_target.symbols,
            &core::array::from_fn::<_, SIZE, _>(|n| BaseField::from_canonical_usize(n + 1)),
        );

        super::SudokuWitnessBuilder::set_sudoku_witness(
            &mut witness,
            &sudoku_problem_target,
            problem,
            solution,
        );

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            circuit
                .prove(witness)
                .expect_err("Invalid witnesses should generate invalid proofs.")
        }))
        .expect_err("Invalid witness inputs");
    }
}
