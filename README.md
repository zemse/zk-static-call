# zk-eth-call

proves result of eth_call

## modified dependencies

the following repositories are forked and this repo uses the `zk-eth-call` branch from those fork. but specific commit is tagged in the Cargo.toml of this project to enable easier rollbacking.

- zkevm-circuits [fork's zk-eth-call branch](https://github.com/zemse/zkevm-circuits/tree/zk-eth-call) - [summary of modifications](#summary-of-modifications)
- axiom-eth [fork's zk-eth-call branch](https://github.com/zemse/axiom-eth)

## instructions

to trigger the proving, just copy paste the following sample call. it executes the `prove` binary that generates traces from local block on anvil (see `inputs_builder.rs`) and submits that to zkevm-circuits (see `prove.rs`).

```
cargo run --release --bin prove -- --to 0x35c6ace6404d8fd1cEe19026B3D56D0C9627a646 --calldata 0x20965255 --block 4363656 --rpc https://eth-sepolia.g.alchemy.com/v2/<ALCHEMY_KEY> --mock
```

## development

to change code in dependencies, clone them in the directory where this project is cloned. and uncomment the "for local development only" part in the Cargo.toml.

```
- parent directory
    - zk-eth-call (this project)
        - README.md (this file)
    - axiom-eth
    - zkevm-circuits
```

## summary of modifications

- add return data to the public inputs
- use axiom-eth for proving init state