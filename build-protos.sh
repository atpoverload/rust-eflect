#!/bin/bash

rm -rf src/protos
rm -rf processing/protos

if [[ "$#" -eq 1 && $1 == 'clean' ]]; then
  exit
fi

protoc --python_out=processing protos/sample/*.proto
