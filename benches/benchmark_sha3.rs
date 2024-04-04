use capycrypt::{Hashable, Message, SecParam};

use capycrypt::sha3::aux_functions::byte_utils::get_random_bytes;
use capycrypt::SecParam::D256;
use criterion::{criterion_group, criterion_main, Criterion};

const BIT_SECURITY: SecParam = D256;

/// hash 5mb of random data with 128 bits of security
fn sha3_digest(mut msg: Message) {
    let mut buf = vec![0_u8; 256 / 8];
    msg.compute_hash_sha3(&BIT_SECURITY, &mut buf);
}

fn bench_sha3_digest(c: &mut Criterion) {
    c.bench_function("SHA3-256 digest 5mb", |b| {
        b.iter(|| sha3_digest(Message::new(get_random_bytes(5242880), D256)));
    });
}

criterion_group!(benches, bench_sha3_digest,);
criterion_main!(benches);
