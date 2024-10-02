#!/usr/bin/env bash

set -e

TARGET_DIR=$1
TARGET_DIR=${TARGET_DIR:-$PWD}

ok=true
for program in solana-{faucet,genesis,keygen,validator}; do
  $program -V || ok=false
done
$ok || {
  echo
  echo "Unable to locate required programs.  Try building them first with:"
  echo
  echo "  $ cargo build --all"
  echo
  exit 1
}

BASE_DIR=$TARGET_DIR/config
if [[ -e $BASE_DIR ]]; then
	echo "Remove $BASE_DIR"
	rm -rf $BASE_DIR
fi

export RUST_BACKTRACE=1
dataDir=$BASE_DIR/"$(basename "$0" .sh)"
ledgerDir=$BASE_DIR/ledger

igloo_RUN_SH_CLUSTER_TYPE=${igloo_RUN_SH_CLUSTER_TYPE:-development}

if ! solana address; then
  echo Generating default keypair
  solana-keygen new --no-passphrase
fi
validator_identity="$dataDir/validator-identity.json"
if [[ -e $validator_identity ]]; then
  echo "Use existing validator keypair"
else
  solana-keygen new --no-passphrase -so "$validator_identity"
fi
validator_vote_account="$dataDir/validator-vote-account.json"
if [[ -e $validator_vote_account ]]; then
  echo "Use existing validator vote account keypair"
else
  solana-keygen new --no-passphrase -so "$validator_vote_account"
fi
validator_stake_account="$dataDir/validator-stake-account.json"
if [[ -e $validator_stake_account ]]; then
  echo "Use existing validator stake account keypair"
else
  solana-keygen new --no-passphrase -so "$validator_stake_account"
fi

solana-genesis \
  --hashes-per-tick sleep \
  --faucet-lamports 500000000000000000 \
  --bootstrap-validator \
    "$validator_identity" \
    "$validator_vote_account" \
    "$validator_stake_account" \
  --ledger "$ledgerDir" \
  --cluster-type "$igloo_RUN_SH_CLUSTER_TYPE"
