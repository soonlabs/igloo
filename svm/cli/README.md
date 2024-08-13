
## Examples
### AccountDB (Bank) Mode
Call the SVM CLI with the path to a Solana program examples:

Hello solana program:
```bash
RUST_LOG=info cargo run -p svm-cli -- -p svm/executor/tests/hello_solana_program.so
```

Clock sysvar program:
```bash
RUST_LOG=info cargo run -p svm-cli -- -p svm/executor/tests/clock_sysvar_program.so
```

Simple transfer program:
```bash
RUST_LOG=info cargo run -p svm-cli -- -p svm/executor/tests/simple_transfer_program.so -c 000000000000000a -a 6nYuNcasWxDxPdNsgCYRev8GwhvdZmBuWsA1vC2NhWKb,900000,true,true -a 442GBBJoU23a92aA3bs9hVkQRxB3SsF3hzbgnjbYetFL,900000,,true -a 11111111111111111111111111111111
```


### Memory Mode
Call the SVM CLI with the path to a Solana program examples:

Hello solana program:
```bash
RUST_LOG=info cargo run -p svm-cli -- -m -p svm/executor/tests/hello_solana_program.so
```

Clock sysvar program:
```bash
RUST_LOG=info cargo run -p svm-cli -- -m -p svm/executor/tests/clock_sysvar_program.so
```

Simple transfer program:
```bash
RUST_LOG=info cargo run -p svm-cli -- -m -p svm/executor/tests/simple_transfer_program.so -c 000000000000000a -a 6nYuNcasWxDxPdNsgCYRev8GwhvdZmBuWsA1vC2NhWKb,900000,true,true -a 442GBBJoU23a92aA3bs9hVkQRxB3SsF3hzbgnjbYetFL,900000,,true -a 11111111111111111111111111111111
```

