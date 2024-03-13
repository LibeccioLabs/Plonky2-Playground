mod dynamic_size_product;
pub use dynamic_size_product::recursive_product_circuit;

mod static_size_product;
pub use static_size_product::ConsecutiveProduct;

const D: usize = 2;
type PGConfig = plonky2::plonk::config::PoseidonGoldilocksConfig;
type BaseField = <PGConfig as plonky2::plonk::config::GenericConfig<D>>::F;
