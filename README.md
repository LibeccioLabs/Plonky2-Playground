# Plonky2 Playground

This crate contains the implementation of two circuits that build a Zero Knowledge Proof:

1. Given a public witness `b` that is a Sudoku board, the circuit is a proof that the prover knows a solution `s` for the Sudoku puzzle.
2. [TODO] Given a public witness `w` and a public constant `k`, the prover knows a secret witness such that `w = n * (n + 1) * (n + 2) * ... * (n + k)`. When `n = 1`, the circuit is a proof that `w` is a factorial number.

This repository tracks our first impact with the
[Plonky2](https://github.com/0xPolygonZero/plonky2) proving system.

A lot of what is written here is heavily inspired to the
[example code](https://github.com/0xPolygonZero/plonky2/tree/main/plonky2/examples)
provided in Plonky2's repository.

## Running the tests

To run the tests, run the command `cargo test --release`.
The command `cargo test` works too, but in that case you may want
to give your computer a couple of minutes to compute the test results.
The flag `-- --nocapture` can be used to print the execution times for proof generation and verification.

The single circuits can be tested by matching the test name with `sudoku`, `permutation` or `factorial`.

### Running via Docker

To run the tests via Docker, the simplest way is to use the image published by CI:

```bash
docker run --rm --network host ghcr.io/libecciolabs/halo2-playground:main
```

You can always build the docker image yourself:

```bash
git clone https://github.com/LibeccioLabs/Plonky2-Playground/
docker build -t plonky2-playground ./Plonky2-Playground/
docker run --rm plonky2-playground
```

Since the Docker image will run `cargo test --release`, you can append any other flag, including a match for the tests you want to run.
For example, to run only the tests in the `sudoku` module, you can run:

```bash
docker run --rm plonky2-playground sudoku
```
