<h1 align="center">
    Igloo
</h1>

<p align="center">
  <a href="https://github.com/soonlabs/igloo/actions/workflows/test.yml">
    <img src="https://github.com/soonlabs/igloo/actions/workflows/test.yml/badge.svg" alt="Ci">
  </a>
  <img src="https://img.shields.io/badge/License-MIT-green.svg?label=license" alt="License">
</p>

## Preface
The popularity of modular blockchains has led to a diversity of Layer 2 blockchain components, with the use of SVM as an execution layer gaining significant attention. The SVM account model decouples computation logic and state, which is highly beneficial for parallel execution. This gives SVM great potential to become a high-performance execution layer. 

In this context, we introduce a high-performance rollup framework, the derivation layer is based on the Optimism design, while the SVM execution layer is based on a new SVM API proposed by Agave. We have decoupled the Solana Transaction Processing Unit (TPU) flow, making the SVM execution layer more lightweight.

Please note this is a demonstration and educational project, please do not use it in a production environment.

## Quick start

We have implemented a simple example based on all definitions mentioned above, to run the example you can use the following command at the root folder:

```bash
RUST_LOG=example=info cargo run -p example
```

or you can use `RUST_LOG=example=trace cargo run -p example`  to see more details.

There is also a simple SVM CLI program in `svm/cli` folder, you can use the following command to call a custom program:

```bash
RUST_LOG=info cargo run -p svm-cli -- -p svm/executor/tests/hello_solana_program.so
```

We’ve added some unit tests (with more to come as we introduce new features). Feel free to use `cargo test` to get more details.

## What is a Rollup?

In simple terms,  rollup derives information from Layer 1 (block headers, deposit transactions, etc.) to Layer 2, then collects transactions from clients, bundles them into blocks, and transmits them to a data availability (DA) provider. Finally, the rollup records the bundled information back on Layer 1.

![Rollup Demo Introduction](https://github.com/user-attachments/assets/b22a2050-3f11-4636-9112-f8fa3c4853e5)

## Rollup-related Definitions

We define all aspects related to rollups using traits combined with generics. The benefit of this approach is that it allows us to provide rollup services for any Layer 1 in the future. Additionally, since our Layer 2 definitions are also abstracted, we should be able to leverage Solana’s high-performance framework to empower any Layer 2.

![Rollup Interface from Introduction](https://github.com/user-attachments/assets/64560c88-426d-4be3-a9b6-c179a00375ba)

### Layer1

We defined general information that is derived from Layer 1, including:

- Layer1 block head info: `L1Head`
- Deposit transaction: `DepositTransaction`
- (Layer2 blocks) Batch info recorded in Layer1: `BatchInfo`
- And a Layer1 block trait including all the above that we are interested: `L1BlockInfo`

Also, an intermediate trait holds data converted to Layer2 block:

```rust
pub trait PayloadAttribute {
    type Transaction: l2::Transaction;
    type Epoch: Epoch;
    type SequenceNumber: Copy;

    fn transactions(&self) -> Arc<Vec<Self::Transaction>>;

    fn epoch_info(&self) -> &Self::Epoch;

    fn sequence_number(&self) -> Self::SequenceNumber;
}
```

### Layer2

We defined general information about a Layer2 block similar to Layer1, including:

- Layer2 block head info: `L2Head`
- Layer2 transactions trait: `Transaction`
- Layer2 transactions batch trait: `Entry`
- And a Layer 2 block trait including all the above: `Block`

#### Engine

One of the most important parts of the Rollups is the interface about the Layer 2 execution layer, which we call the engine:

```rust
pub trait Engine: EngineApi<Self::Block, Self::Head> {
    type TransactionStream: stream::TransactionStream;
    type Payload: BlockPayload;
    type Head: L2Head;
    type Block: Block<Head = Self::Head>;
    type BlockHeight: Copy;

    fn stream(&self) -> &Arc<RwLock<Self::TransactionStream>>;

    async fn get_head(
        &mut self,
        height: Self::BlockHeight,
    ) -> Result<Option<Self::Head>, Self::Error>;
}

pub trait EngineApi<B: Block, H: L2Head> {
    type Error;

    async fn new_block(&mut self, block: B) -> Result<H, Self::Error>;

    async fn reorg(&mut self, reset_to: H) -> Result<(), Self::Error>;

    async fn finalize(&mut self, block: H) -> Result<(), Self::Error>;
}
```

From the codes above we can see an `EngineApi` trait that defines the behaviors to create a new block and handle according to Layer1 consensus events, and an `Engine` trait with a more general purpose.

#### Stream

We aim to manage all Layer 2 consensus processes, which necessitates a transaction server to respond to all Layer 2 clients. Additionally, we’ll need a Gulf Stream-like mechanism, similar to a transaction pool:

```rust
pub trait TransactionStream {
    type TxIn: Transaction;
    type TxOut: Transaction;
    type Settings: BatchSettings;
    type Error;

    async fn insert(&mut self, tx: Self::TxIn) -> Result<(), Self::Error>;

    async fn next_batch(&mut self, settings: Self::Settings) -> Vec<Self::TxOut>;
}
```

### General process

#### Derives

The derivation process involves extracting information from Layer 1, and there are two types of derivations: instant derivation and DA derivation. Accordingly, we have two derivation traits: `InstantDerive` and `DaDerive`.

`InstantDerive` is a trait that can be implemented by a struct to instantly derive a new block from an L1 block using logs (events).

```rust
pub trait InstantDerive {
    type P: PayloadAttribute;
    type L1Info: L1BlockInfo<Self::P>;
    type Error;

    /// Try to derive a new block from the L1 block, return `None` if
    ///  there is no new block to derive.
    async fn get_new_block(&mut self) -> Result<Option<Self::L1Info>, Self::Error>;
}
```

`DaDerive` is a trait that can be implemented by a struct to derive blocks from a DA provider.

```rust
pub trait DaDerive {
    type Item: PayloadAttribute;

    /// Fetch next `PayloadAttribute` from DA provider. This method
    /// is similar to `Iterator::next` but in async manner.
    async fn next(&mut self) -> Option<Self::Item>;
}
```

#### Runner

The most high-level interface is the `Runner` trait that integrates the entire workflow:

```rust
pub trait Runner<E: Engine, ID: InstantDerive, DD: DaDerive> {
    type Error;

    fn register_instant(&mut self, derive: ID);

    fn register_da(&mut self, derive: DD);

    fn get_engine(&self) -> &E;

    async fn advance(&mut self) -> Result<(), Self::Error>;
}
```

From the interface above, we can see that the `Runner` holds an `Engine` to handle the actual block production work. It can register one or more `InstantDerive` and `DaDerive` objects. The `advance` action acts as a tick, triggered each time Layer 2 needs to mine a new block.

## SVM Executor

We implement a Solana Virtual Machine (SVM) executor based on [Agave SVM](https://github.com/anza-xyz/agave/tree/master/svm) for rollups, we have done some work including:

- Isolation of changes at [Agave SVM](https://github.com/anza-xyz/agave/tree/master/svm): The Agave team has done an excellent job with the SVM and continues to make great progress. As the SVM-related logic evolves rapidly, we need to add a buffer layer to minimize the impact on our framework.
- EVM environments preparing: [Agave SVM](https://github.com/anza-xyz/agave/tree/master/svm) is implemented for general purpose, we need some extra jobs to execute out Layer1 transactions
- Execution builder: We designed execution builders to collect information for SVM calls, such as preparing accounts, loading programs, setting calldata etc. That is very useful for testing and single transaction execution (such as a SVM Cli program), and we plan to develop more powerful full builders combined with rollup storage and distributed execution in future.
- More bank-related interfaces: We defined some extra interfaces used during our rollup process, and also adapted to our rollup storage later.

## What’s Next

### Standalone Storage

We plan to combine Solana’s high-performance storage systems, `Blockstore` and `AccountDb`, and abstract a `Ledger` interface to handle Layer 2 blocks, tailored for those familiar with Bitcoin and Ethereum-style ledger structures.

Additionally, we’ll introduce interfaces for Layer 2 consensus, such as handling reorgs after Layer 1 chain forks and finalizing blocks once they’ve been settled on Layer 1.

### Parallel Execution

We’ll leverage Solana’s new scheduler mechanism in the Bank to implement parallel execution in the SVM, along with introducing more powerful SVM executor builders.

### Decentralized rollup

We have defined the behaviors for a centralized rollup. To implement a decentralized rollup framework, the first step is to add Layer 2 node synchronization logic using P2P technology. We may introduce multiple Layer 2 block producers (also known as sequencers) at a later stage.
