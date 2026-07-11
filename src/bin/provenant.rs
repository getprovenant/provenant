// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

// musl's default allocator serializes badly under the scanner's highly parallel
// allocation, making static musl builds orders of magnitude slower. mimalloc restores
// glibc-class throughput. Only the musl release binaries are affected; glibc, macOS, and
// Windows builds keep the system allocator.
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    if let Err(err) = provenant::cli::run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
