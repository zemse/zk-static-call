# zk-eth-call

proves result of eth_call

## modified dependencies

the following repositories are forked and this repo uses the `zk-eth-call` branch from those fork. but specific commit is tagged in the Cargo.toml of this project to enable easier rollbacking.

- zkevm-circuits [fork's zk-eth-call branch](https://github.com/zemse/zkevm-circuits/tree/zk-eth-call)
- axiom-eth [fork's zk-eth-call branch](https://github.com/zemse/axiom-eth)

## instructions

to trigger the proving, just copy paste the following sample call. it executes the `prove` binary that generates traces from local block on anvil (see `inputs_builder.rs`) and submits that to zkevm-circuits (see `prove.rs`).

```
cargo run --release --bin prove -- --mock --rpc-url https://eth-sepolia.g.alchemy.com/v2/f-R85PXVLHxyAfQu5cngt47PYzOaJ99m --fork-block 3147881 --raw-tx 0xf88c8084ee6b28008301388094df03add8bc8046df3b74a538c57c130cefb89b8680a46057361d00000000000000000000000000000000000000000000000000000000000000018401546d72a0f5b7e54553deeb044429b394595581501209a627beef020e764426aa0955e93aa00927cb7de78c15d2715de9a5cbde171c7202755864656cd4726ac43c76a9000a --max-keccak-rows 50000
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