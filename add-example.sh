#!/bin/bash

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <name>"
    exit 1
fi

NAME=$1

echo "[[example]]" >> ./examples/Cargo.toml
echo "name = \"$NAME\"" >> ./examples/Cargo.toml
echo "path = \"$NAME.rs\"" >> ./examples/Cargo.toml

cat <<EOF >./examples/$NAME.rs
#[tokio::main]
async fn main() {}
EOF
