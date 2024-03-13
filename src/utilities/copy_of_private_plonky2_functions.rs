use plonky2::{
    field::extension::Extendable,
    hash::hash_types::{HashOutTarget, MerkleCapTarget, RichField},
    iop::target::Target,
    plonk::circuit_data::{CommonCircuitData, VerifierCircuitTarget},
};

/// This is a copy of the function
/// `plonky2::recursion::cyclic_recursion::VerifierCircuitTarget::from_slice`
/// which is not publicly visible, but there would be no harm if it was.
///
/// TODO: open an issue about it and link it here
pub fn from_slice<F: RichField + Extendable<D>, const D: usize>(
    slice: &[Target],
    common_data: &CommonCircuitData<F, D>,
) -> anyhow::Result<VerifierCircuitTarget> {
    let cap_len = common_data.config.fri_config.num_cap_elements();
    let len = slice.len();
    anyhow::ensure!(len >= 4 + 4 * cap_len, "Not enough public inputs");
    let constants_sigmas_cap = MerkleCapTarget(
        (0..cap_len)
            .map(|i| HashOutTarget {
                elements: core::array::from_fn(|j| slice[len - 4 * (cap_len - i) + j]),
            })
            .collect(),
    );
    let circuit_digest = HashOutTarget {
        elements: core::array::from_fn(|i| slice[len - 4 - 4 * cap_len + i]),
    };

    Ok(VerifierCircuitTarget {
        circuit_digest,
        constants_sigmas_cap,
    })
}
