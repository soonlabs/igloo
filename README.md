## How to start

Run the following command to start the rollup example:
```
RUST_LOG=example=info cargo run -p example
```

or you can use `RUST_LOG=example=trace` to see more logs.

## Directory Structure

<pre>
├── <a href="./interface">interface</a>: Rollup related interfaces.
├── <a href="./example">example</a>: Basic example of a rollup implementation.
├── <a href="./svm/executor">svm executor</a>: Svm executor to execute solana programs.
├── <a href="./svm/cli">svm executor</a>: Svm cli tool to run solana programs.
