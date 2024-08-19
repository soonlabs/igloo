## How to start

Run the following command to start the rollup example:
```
RUST_LOG=example=info cargo run -p example
```

or you can use `RUST_LOG=example=trace` to see more logs.

To run a program with svm cli please ref to <a href="./svm/cli">cli readme</a>

## Directory Structure

<pre>
├── <a href="./example">example</a>: Basic example of a rollup implementation.
├── <a href="./interface">interface</a>: Rollup related interfaces.
├── <a href="./svm">svm</a>
    ├── <a href="./svm/cli">/cli</a>: Svm cli tool to run solana programs.
    ├── <a href="./svm/executor">/executor</a>: Svm executor to execute solana programs.
