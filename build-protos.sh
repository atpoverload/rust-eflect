#!/bin/bash

rm -rf src/protos
rm -rf processing/protos

if [[ "$#" -eq 1 && $1 == 'clean' ]]; then
  exit
fi

mkdir -p src/protos
protoc --python_out=processing --rust_out=src/protos protos/sample/*.proto
echo "pub mod sample;
pub mod jiffies;
pub mod rapl;" > src/protos/mod.rs
