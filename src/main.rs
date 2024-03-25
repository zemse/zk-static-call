#![feature(trivial_bounds)]
mod builder;
mod circuit;
mod common;
mod init_state;

use halo2_proofs::dev::MockProver;

fn main() {
    let (k, circuit, instances) = circuit::new();

    println!("running circuit");
    let prover = MockProver::run(k, &circuit, instances).unwrap();
    println!("verifying constraints");
    prover.assert_satisfied_par();
    println!("success!");
}
